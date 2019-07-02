//! Node debugger

use std::io;
use std::path::Path;
use std::process::{exit, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::debugger::node::ws::WSHandler;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};
use tokio_process::CommandExt;
use websocket::OwnedMessage;

#[derive(Debug)]
struct FileLocation {
    file: String,
    line_num: u64,
}

#[derive(Debug)]
struct Script {
    file: String,
    script_id: String,
    is_internal: bool,
}

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    node_process: Option<Command>,
    ws_handler: Arc<Mutex<WSHandler>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    scripts: Arc<Mutex<Vec<Script>>>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        let ws_handler = WSHandler::new(notifier.clone());
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            node_process: None,
            ws_handler: Arc::new(Mutex::new(ws_handler)),
            pending_breakpoints: Arc::new(Mutex::new(vec![])),
            scripts: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {}

    fn teardown(&mut self) {
        exit(0);
    }

    fn has_started(&self) -> bool {
        true
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let mut cmd = Command::new(self.debugger_cmd.clone())
            .arg(format!("--inspect-brk=0"))
            .arg("--")
            .args(self.run_cmd.clone())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_async()
            .expect("Can't spawn node");

        let mut node_stdin = cmd.stdin().take().unwrap();
        let node_stderr = cmd.stderr().take().unwrap();

        thread::spawn(move || {
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                node_stdin.write(&buf).unwrap();
            }
        });

        let reader = io::BufReader::new(node_stderr);
        let lines = tokio::io::lines(reader);
        let (has_started_tx, has_started_rx) = mpsc::channel(1);

        tokio::spawn(
            lines
                .for_each(move |line| {
                    eprintln!("{}", line);

                    match check_line_debugger_started(line) {
                        Some(debugger_uri) => {
                            tokio::spawn(
                                has_started_tx
                                    .clone()
                                    .send(debugger_uri)
                                    .map(|_| {})
                                    .map_err(|e| {
                                        eprintln!("Error spawning node: {:?}", e);
                                    }),
                            );
                        }
                        None => {}
                    };

                    Ok(())
                })
                .map_err(|e| println!("stderr err: {:?}", e)),
        );

        let notifier = self.notifier.clone();

        tokio::spawn(
            cmd.map(move |exit_status| {
                notifier
                    .lock()
                    .unwrap()
                    .signal_exited(0, exit_status.code().unwrap() as i64);
            })
            .map_err(|e| {
                eprintln!("Error spawning node: {}", e);
            }),
        );

        let scripts = self.scripts.clone();
        let pending_breakpoints = self.pending_breakpoints.clone();
        let notifier = self.notifier.clone();
        let notifier2 = self.notifier.clone();

        let ws_handler = self.ws_handler.clone();
        let ws_handler2 = self.ws_handler.clone();

        let (execution_context_created_tx, execution_context_created_rx) = mpsc::channel(1);

        // TODO:
        let f = has_started_rx
            .take(1)
            .into_future()
            .map(move |uri| {
                // We need a little sleep otherwise we fail to connect,
                // shame to block the thread but can live with it while
                // starting up process
                thread::sleep(Duration::new(2, 0));

                let ws_handler_analyser = ws_handler.clone();
                let scripts_analyser = scripts.clone();
                let pending_breakpoints_analyser = pending_breakpoints.clone();
                let notifier_analyser = notifier.clone();

                ws_handler
                    .lock()
                    .unwrap()
                    .connect(&uri.0.unwrap(), move |msg| {
                        analyse_message(
                            msg,
                            execution_context_created_tx.clone(),
                            ws_handler_analyser.clone(),
                            scripts_analyser.clone(),
                            pending_breakpoints_analyser.clone(),
                            notifier_analyser.clone(),
                        );
                        None
                    });
            })
            .then(move |_| {
                let msg = OwnedMessage::Text("{\"method\":\"Runtime.enable\"}".to_string());
                let f1 = ws_handler2
                    .clone()
                    .lock()
                    .unwrap()
                    .send_and_receive_message(msg);
                let msg = OwnedMessage::Text("{\"method\":\"Debugger.enable\"}".to_string());
                let f2 = ws_handler2.lock().unwrap().send_and_receive_message(msg);
                let msg = OwnedMessage::Text(
                    "{\"method\":\"Runtime.runIfWaitingForDebugger\"}".to_string(),
                );
                let f3 = ws_handler2.lock().unwrap().send_and_receive_message(msg);

                f1.join(f2).join(f3)
            })
            .then(move |responses| {
                execution_context_created_rx
                    .into_future()
                    .map(move |context_response| {
                        let responses = responses.unwrap();
                        let resp1 = (responses.0).0.clone();
                        let resp2 = (responses.0).1.clone();
                        let resp3 = responses.1.clone();
                        if resp1["error"].is_null()
                            && resp2["error"].is_null()
                            && resp3["error"].is_null()
                        {
                            let mut pid_str: String =
                                match serde_json::from_value(context_response.0.unwrap()) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        panic!("Can't understand pid: {:?}", e);
                                    }
                                };
                            let to = pid_str.len() - 1;
                            pid_str = pid_str[5..to].to_string();

                            (true, pid_str)
                        } else {
                            notifier2.clone().lock().unwrap().log_msg(
                                LogLevel::ERROR,
                                format!(
                                    "Error received from node: {:?} {:?}",
                                    responses, context_response
                                ),
                            );
                            (false, "".to_string())
                        }
                    })
                    .map_err(|e| {
                        eprintln!("Error sending to node: {:?}", e.0);
                        io::Error::new(io::ErrorKind::Other, "Timed out sending to node")
                    })
            })
            .map(|pid_data| {
                let resp;
                if pid_data.0 {
                    resp = serde_json::json!({"status":"OK","pid":pid_data.1});
                } else {
                    resp = serde_json::json!({"status":"ERROR"});
                }
                resp
            })
            .map_err(|e| {
                eprintln!("Error connecting websocket to node: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out connecting")
            });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file: String,
        line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let full_file_name = Path::new(&file).canonicalize();
        match full_file_name {
            Ok(s) => {
                for script in self.scripts.lock().unwrap().iter() {
                    let file = s.to_string_lossy().to_string();

                    if script.file == file {
                        let msg = OwnedMessage::Text(format!(
                            "{{\
                             \"method\":\"Debugger.setBreakpoint\",\
                             \"params\":{{\
                             \"location\":{{\
                             \"scriptId\":\"{}\",\
                             \"lineNumber\":{}\
                             }}\
                             }}\
                             }}",
                            script.script_id,
                            line_num - 1
                        ));

                        let notifier = self.notifier.clone();

                        let f = self
                            .ws_handler
                            .lock()
                            .unwrap()
                            .send_and_receive_message(msg)
                            .map(move |response| {
                                if response["error"].is_null() {
                                    notifier
                                        .lock()
                                        .unwrap()
                                        .breakpoint_set(file.clone(), line_num);

                                    serde_json::json!({"status":"OK"})
                                } else {
                                    serde_json::json!({"status":"ERROR"})
                                }
                            });

                        return Box::new(f);
                    }
                }

                let file = s.to_string_lossy().to_string();

                self.pending_breakpoints
                    .lock()
                    .unwrap()
                    .push(FileLocation { file, line_num });
            }
            Err(e) => {
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::ERROR, format!("Can't find file {}: {}", file, e));

                let f = future::lazy(move || {
                    let resp = serde_json::json!({"status":"ERROR"});
                    Ok(resp)
                });

                return Box::new(f);
            }
        };

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"PENDING"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let msg = OwnedMessage::Text("{\"method\":\"Debugger.stepInto\"}".to_string());

        let f = self
            .ws_handler
            .lock()
            .unwrap()
            .send_and_receive_message(msg)
            .map(|response| {
                if response["error"].is_null() {
                    serde_json::json!({"status":"OK"})
                } else {
                    serde_json::json!({"status":"ERROR"})
                }
            });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let msg = OwnedMessage::Text("{\"method\":\"Debugger.stepOver\"}".to_string());

        let f = self
            .ws_handler
            .lock()
            .unwrap()
            .send_and_receive_message(msg)
            .map(|response| {
                if response["error"].is_null() {
                    serde_json::json!({"status":"OK"})
                } else {
                    serde_json::json!({"status":"ERROR"})
                }
            });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let msg = OwnedMessage::Text("{\"method\":\"Debugger.resume\"}".to_string());

        let f = self
            .ws_handler
            .lock()
            .unwrap()
            .send_and_receive_message(msg)
            .map(|response| {
                if response["error"].is_null() {
                    serde_json::json!({"status":"OK"})
                } else {
                    serde_json::json!({"status":"ERROR"})
                }
            });

        Box::new(f)
    }

    fn print(
        &mut self,
        variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let msg = OwnedMessage::Text(format!(
            "{{\
             \"method\":\"Debugger.evaluateOnCallFrame\",\
             \"params\":{{\
             \"callFrameId\":\"{{\\\"ordinal\\\":0,\\\"injectedScriptId\\\":1}}\",\
             \"expression\":\"{}\",\
             \"returnByValue\":true\
             }}\
             }}",
            variable,
        ));

        let variable = variable.to_string();

        let f = self
            .ws_handler
            .lock()
            .unwrap()
            .send_and_receive_message(msg)
            .map(move |response| {
                println!("Response: {:?}", response);
                if response["error"].is_null() {
                    let mut json = response;
                    let variable_type = json["result"]["result"]["type"].take();
                    let value = json["result"]["result"]["value"].take();
                    serde_json::json!({
                        "status": "OK",
                        "type": variable_type,
                        "variable": variable,
                        "value": value,
                    })
                } else {
                    serde_json::json!({"status":"ERROR"})
                }
            });

        Box::new(f)
    }
}

fn check_line_debugger_started(line: String) -> Option<String> {
    lazy_static! {
        static ref RE_NODE_STARTED: Regex =
            Regex::new("^Debugger listening on (ws://127.0.0.1:\\d+/.*)$").unwrap();
    }

    for cap in RE_NODE_STARTED.captures_iter(&line) {
        return Some(cap[1].to_string());
    }

    None
}

fn analyse_message(
    json: serde_json::Value,
    execution_context_created_tx: Sender<serde_json::Value>,
    ws_handler: Arc<Mutex<WSHandler>>,
    scripts: Arc<Mutex<Vec<Script>>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let method = json["method"].clone();
    let method: String = match serde_json::from_value(method) {
        Ok(s) => s,
        Err(e) => {
            panic!("Can't understand method: {:?}", e);
        }
    };

    if method == "Debugger.scriptParsed" {
        analyse_script_parsed(
            json,
            ws_handler.clone(),
            scripts.clone(),
            pending_breakpoints.clone(),
            notifier.clone(),
        );
    } else if method == "Runtime.executionContextCreated" {
        tokio::spawn(
            execution_context_created_tx
                .send(json["params"]["context"]["name"].clone())
                .map(|_| {})
                .map_err(|e| {
                    eprintln!("Error spawning node: {:?}", e);
                }),
        );
    } else if method == "Debugger.paused" {
        analyse_debugger_paused(json, scripts, notifier.clone());
    } else if method == "Debugger.resumed" {
        println!("TODO: Code {:?}", json);
    } else if method == "Runtime.consoleAPICalled" {
    } else if method == "Runtime.exceptionThrown" {
        println!("TODO: Code {:?}", json);
    } else if method == "Runtime.executionContextDestroyed" {
        ws_handler.lock().unwrap().close();
    } else {
        panic!("Can't understand message: {:?}", json);
    }
}

fn analyse_script_parsed(
    json: serde_json::Value,
    ws_handler: Arc<Mutex<WSHandler>>,
    scripts: Arc<Mutex<Vec<Script>>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let mut is_internal = false;

    let mut json = json;

    let file = json["params"]["url"].take();
    let file: String = match serde_json::from_value(file.clone()) {
        Ok(s) => {
            let mut s: String = s;
            if s.len() > 7 && &s[0..7] == "file://" {
                is_internal = true;
                s = s[7..].to_string()
            }
            s
        }
        Err(e) => {
            panic!("Can't understand file: {:?}", e);
        }
    };

    let script_id = json["params"]["scriptId"].take();
    let script_id: String = match serde_json::from_value(script_id.clone()) {
        Ok(s) => s,
        Err(e) => {
            panic!("Can't understand script_id: {:?}", e);
        }
    };

    for bkpt in pending_breakpoints.lock().unwrap().iter() {
        if bkpt.file == file {
            let msg = OwnedMessage::Text(
                format!("{{\"method\":\"Debugger.setBreakpoint\",\"params\":{{\"location\":{{\"scriptId\":\"{}\",\"lineNumber\":{}}}}}}}", script_id, bkpt.line_num - 1)
            );

            let notifier_breakpoint = notifier.clone();
            let file = file.clone();
            let line_num = bkpt.line_num;

            tokio::spawn(
                ws_handler
                    .lock()
                    .unwrap()
                    .send_and_receive_message(msg)
                    .map(move |response| {
                        if response["error"].is_null() {
                            notifier_breakpoint
                                .lock()
                                .unwrap()
                                .breakpoint_set(file, line_num);
                        }

                        ()
                    })
                    .map_err(|e| {
                        eprintln!("Error setting pending breakpoint: {}", e);
                        ()
                    }),
            );
        }
    }

    scripts.lock().unwrap().push(Script {
        file,
        script_id,
        is_internal,
    });
}

fn analyse_debugger_paused(
    mut json: serde_json::Value,
    _scripts: Arc<Mutex<Vec<Script>>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let file = json["params"]["callFrames"][0]["url"].take();
    let file: String = match serde_json::from_value(file) {
        Ok(s) => {
            let mut s: String = s;
            if s.len() > 7 && &s[0..7] == "file://" {
                s = s[7..].to_string()
            }
            s
        }
        Err(e) => {
            println!("JSON: {}", json);
            let result = json["result"].take();
            match result {
                serde_json::Value::Null => {
                    println!("HERE");
                }
                _ => {
                    println!("HERE2");
                }
            }
            panic!("Err TODO Code: {:?}", e);
        }
    };

    let line_num = json["params"]["callFrames"][0]["location"]["lineNumber"].take();
    let line_num: u64 = match serde_json::from_value(line_num) {
        Ok(s) => {
            let s: u64 = s;
            s + 1
        }
        Err(e) => {
            panic!("Can't understand line_num: {:?}", e);
        }
    };

    notifier.lock().unwrap().jump_to_position(file, line_num);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::debugger::node::ws::WSHandler;
    use crate::notifier;
    use tokio::sync::mpsc;

    #[test]
    fn check_internal_script_parsed() {
        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params": {
                "scriptId":"7",
                "url":"internal/bootstrap/loaders.js",
                "startLine":0,
                "startColumn":0,
                "endLine":312,
                "endColumn":0,
                "executionContextId":1,
                "hash":"39ff95c38ab7c4bb459aabfe5c5eb3a27441a4d8",
                "executionContextAuxData":{
                  "isDefault":true
                },
                "isLiveEdit":false,
                "sourceMapURL":"",
                "hasSourceURL":false,
                "isModule":false,
                "length":9613
              }
            }
        );
        let (tx, _) = mpsc::channel(1);
        let scripts = Arc::new(Mutex::new(vec![]));
        let pending_breakpoints: Arc<Mutex<Vec<super::FileLocation>>> =
            Arc::new(Mutex::new(vec![]));
        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));
        let ws_handler = Arc::new(Mutex::new(WSHandler::new(notifier.clone())));

        super::analyse_message(
            msg,
            tx.clone(),
            ws_handler.clone(),
            scripts.clone(),
            pending_breakpoints.clone(),
            notifier.clone(),
        );

        assert_eq!(scripts.clone().lock().unwrap().len(), 1);
        assert_eq!(
            scripts.clone().lock().unwrap()[0].file,
            "internal/bootstrap/loaders.js".to_string()
        );
        assert_eq!(
            scripts.clone().lock().unwrap()[0].script_id,
            "7".to_string()
        );
        assert_eq!(scripts.clone().lock().unwrap()[0].is_internal, false);

        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params":{
                "scriptId":"8",
                "url":"internal/bootstrap/node.js",
                "startLine":0,
                "startColumn":0,
                "endLine":438,
                "endColumn":0,
                "executionContextId":1,
                "hash":"3f184a9d8a71f2554b8b31895d935027129c91c4",
                "executionContextAuxData":{
                  "isDefault":true
                },
                "isLiveEdit":false,
                "sourceMapURL":"",
                "hasSourceURL":false,
                "isModule":false,
                "length":14904
              }
            }
        );

        super::analyse_message(
            msg,
            tx,
            ws_handler,
            scripts.clone(),
            pending_breakpoints,
            notifier,
        );

        assert_eq!(scripts.clone().lock().unwrap().len(), 2);
        assert_eq!(
            scripts.clone().lock().unwrap()[1].file,
            "internal/bootstrap/node.js".to_string()
        );
        assert_eq!(
            scripts.clone().lock().unwrap()[1].script_id,
            "8".to_string()
        );
        assert_eq!(scripts.lock().unwrap()[1].is_internal, false);
    }

    #[test]
    fn check_file_script_parsed() {
        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params":{
                "scriptId":"52",
                "url":"file:///home/me/test.js"
              }
            }
        );
        let (tx, _) = mpsc::channel(1);
        let scripts = Arc::new(Mutex::new(vec![]));
        let pending_breakpoints: Arc<Mutex<Vec<super::FileLocation>>> =
            Arc::new(Mutex::new(vec![]));
        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));
        let ws_handler = Arc::new(Mutex::new(WSHandler::new(notifier.clone())));

        super::analyse_message(
            msg,
            tx,
            ws_handler,
            scripts.clone(),
            pending_breakpoints,
            notifier,
        );

        assert_eq!(scripts.clone().lock().unwrap().len(), 1);
        assert_eq!(
            scripts.clone().lock().unwrap()[0].file,
            "/home/me/test.js".to_string()
        );
        assert_eq!(
            scripts.clone().lock().unwrap()[0].script_id,
            "52".to_string()
        );
        assert_eq!(scripts.clone().lock().unwrap()[0].is_internal, true);
    }
}
