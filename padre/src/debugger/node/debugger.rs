//! Node debugger

use std::io;
use std::path::Path;
use std::process::{exit, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};
use crate::util;

use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};
use tokio_process::CommandExt;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};

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
    ws_tx: Arc<Mutex<Option<Sender<OwnedMessage>>>>,
    ws_id: Arc<Mutex<u64>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    scripts: Arc<Mutex<Vec<Script>>>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            node_process: None,
            ws_tx: Arc::new(Mutex::new(None)),
            ws_id: Arc::new(Mutex::new(1)),
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
        let port = util::get_unused_localhost_port();
        let mut cmd = Command::new(self.debugger_cmd.clone())
            .arg(format!("--inspect-brk={}", port))
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
        let ws_tx = self.ws_tx.clone();
        let ws_id = self.ws_id.clone();
        let scripts = self.scripts.clone();
        let pending_breakpoints = self.pending_breakpoints.clone();
        let notifier = self.notifier.clone();

        let (otx, orx) = mpsc::channel(1);

        tokio::spawn(
            lines
                .for_each(move |line| {
                    eprintln!("{}", line);

                    analyse_line(
                        line,
                        port,
                        ws_tx.clone(),
                        ws_id.clone(),
                        otx.clone(),
                        scripts.clone(),
                        pending_breakpoints.clone(),
                        notifier.clone(),
                    );

                    Ok(())
                })
                .map_err(|e| println!("stderr err: {:?}", e)),
        );

        tokio::spawn(
            cmd.map(|a| {
                println!("process: {}", a);
            })
            .map_err(|e| {
                eprintln!("Error spawning node: {}", e);
            }),
        );

        let f = orx
            .take(1)
            .into_future()
            .map(|ok| {
                let resp;
                if ok.0.unwrap() {
                    resp = serde_json::json!({"status":"OK"});
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
                        let ws_tx = self.ws_tx.clone();

                        let tx = ws_tx.lock().unwrap().clone().unwrap();

                        let f = tx.send(OwnedMessage::Text(
                                format!("{{\"method\":\"Debugger.setBreakpoint\",\"params\":{{\"location\":{{\"scriptId\":{},\"lineNumber\":{}}}}}}}", script.script_id, line_num - 1)
                            )).map(|_| {
                                serde_json::json!({"status":"OK"})
                            }).map_err(|e| {
                                eprintln!("Error sending breakpoint to node: {:?}", e);
                                io::Error::new(io::ErrorKind::Other, "Timed out sending breakpoint")
                            });

                        self.notifier
                            .lock()
                            .unwrap()
                            .breakpoint_set(file.clone(), line_num);

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
        let ws_tx = self.ws_tx.lock().unwrap().clone().unwrap();

        let f = ws_tx
            .send(OwnedMessage::Text(
                "{\"method\":\"Debugger.stepInto\"}".to_string(),
            ))
            .map(|_| serde_json::json!({"status":"OK"}))
            .map_err(|e| {
                eprintln!("Error sending stepping in: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out stepping in")
            });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let ws_tx = self.ws_tx.lock().unwrap().clone().unwrap();

        let f = ws_tx
            .send(OwnedMessage::Text(
                "{\"method\":\"Debugger.stepOver\"}".to_string(),
            ))
            .map(|_| serde_json::json!({"status":"OK"}))
            .map_err(|e| {
                eprintln!("Error sending stepping in: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out stepping in")
            });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let ws_tx = self.ws_tx.lock().unwrap().clone().unwrap();

        let f = ws_tx
            .send(OwnedMessage::Text(
                "{\"method\":\"Debugger.resume\"}".to_string(),
            ))
            .map(|_| serde_json::json!({"status":"OK"}))
            .map_err(|e| {
                eprintln!("Error sending stepping in: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out stepping in")
            });

        Box::new(f)
    }

    fn print(
        &mut self,
        _variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }
}

fn analyse_line(
    line: String,
    port: u16,
    ws_tx: Arc<Mutex<Option<Sender<OwnedMessage>>>>,
    ws_id: Arc<Mutex<u64>>,
    otx: Sender<bool>,
    scripts: Arc<Mutex<Vec<Script>>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    lazy_static! {
        static ref RE_NODE_STARTED: Regex =
            Regex::new("^Debugger listening on ws://127.0.0.1:\\d+/(.*)$").unwrap();
    }

    for cap in RE_NODE_STARTED.captures_iter(&line) {
        let node_debugger_hex = cap[1].to_string();
        let uri = format!("ws://localhost:{}/{}", port, node_debugger_hex);

        // We need a little sleep otherwise we fail to connect,
        // shame to block the thread but can live with it while
        // starting up process
        thread::sleep(Duration::new(2, 0));

        let (tx, rx) = mpsc::channel(1);
        *ws_tx.clone().lock().unwrap() = Some(tx.clone());
        let ws_id = ws_id.clone();
        let otx = otx.clone();
        let scripts = scripts.clone();
        let pending_breakpoints = pending_breakpoints.clone();
        let notifier = notifier.clone();

        let f = ClientBuilder::new(&uri)
            .unwrap()
            .async_connect_insecure()
            .and_then(move |(duplex, _)| {
                let (sink, stream) = duplex.split();

                let tx_setup = tx.clone();
                let scripts = scripts.clone();

                tokio::spawn(
                    tx_setup
                        .clone()
                        .send(OwnedMessage::Text(
                            "{\"method\":\"Runtime.enable\"}".to_string(),
                        ))
                        .map(|a| {
                            println!("Sending setup: {:?}", a);
                        })
                        .map_err(|e| {
                            println!("Error sending setup: {:?}", e);
                        }),
                );

                tokio::spawn(
                    tx_setup
                        .clone()
                        .send(OwnedMessage::Text(
                            "{\"method\":\"Debugger.enable\"}".to_string(),
                        ))
                        .map(|a| {
                            println!("Sending setup: {:?}", a);
                        })
                        .map_err(|e| {
                            println!("Error sending setup: {:?}", e);
                        }),
                );

                tokio::spawn(
                    tx_setup
                        .clone()
                        .send(OwnedMessage::Text(
                            "{\"method\":\"Runtime.runIfWaitingForDebugger\"}".to_string(),
                        ))
                        .map(move |a| {
                            println!("Sending setup: {:?}", a);
                        })
                        .map_err(|e| {
                            println!("Error sending setup: {:?}", e);
                        }),
                );

                let ws_tx = tx.clone();

                stream
                    .filter_map(move |message| {
                        analyse_message(
                            message,
                            ws_tx.clone(),
                            otx.clone(),
                            scripts.clone(),
                            pending_breakpoints.clone(),
                            notifier.clone(),
                        );

                        None
                    })
                    .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
                    .map(move |msg| add_id_to_message(msg, ws_id.clone()))
                    .forward(sink)
            })
            .map(|_| ())
            .map_err(|e| eprintln!("WebSocket err: {:?}", e));

        tokio::spawn(f);
    }
}

fn analyse_message(
    message: OwnedMessage,
    ws_tx: Sender<OwnedMessage>,
    otx: Sender<bool>,
    scripts: Arc<Mutex<Vec<Script>>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let mut json: serde_json::Value;
    if let OwnedMessage::Text(s) = &message {
        json = serde_json::from_str(s).unwrap();
    } else {
        panic!("Can't understand message: {:?}", message)
    }

    if json["method"].is_string() {
        let method = json["method"].take();
        let method: String = match serde_json::from_value(method.clone()) {
            Ok(s) => s,
            Err(e) => {
                panic!("Can't understand method: {:?}", e);
            }
        };

        if method == "Debugger.scriptParsed" {
            analyse_script_parsed(
                json,
                ws_tx.clone(),
                scripts.clone(),
                pending_breakpoints.clone(),
                notifier.clone(),
            );
        } else if method == "Runtime.executionContextCreated" {
            tokio::spawn(otx.clone().send(true).map(|_| {}).map_err(|e| {
                eprintln!("Error spawning node: {:?}", e);
            }));
        } else if method == "Debugger.paused" {
            analyse_debugger_paused(json, notifier.clone());
        } else if method == "Runtime.exceptionThrown" {
            println!("TODO: Code {:?}", message);
        } else {
            panic!("Can't understand message: {:?}", message);
        }
    } else if json["id"].is_number() {
        println!("Handling id {}", json["id"]);
    } else {
        panic!("Can't understand message: {:?}", json)
    }
}

fn analyse_script_parsed(
    json: serde_json::Value,
    ws_tx: Sender<OwnedMessage>,
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
            tokio::spawn(
                ws_tx.clone().send(OwnedMessage::Text(
                    format!("{{\"method\":\"Debugger.setBreakpoint\",\"params\":{{\"location\":{{\"scriptId\":{},\"lineNumber\":{}}}}}}}", script_id, bkpt.line_num - 1)
                )).map(|_| {
                }).map_err(|e| {
                    eprintln!("Error sending breakpoint to node: {:?}", e);
                })
            );

            notifier
                .lock()
                .unwrap()
                .breakpoint_set(file.clone(), bkpt.line_num);
        }
    }

    scripts.lock().unwrap().push(Script {
        file,
        script_id,
        is_internal,
    });
}

fn analyse_debugger_paused(mut json: serde_json::Value, notifier: Arc<Mutex<Notifier>>) {
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
            panic!("Can't understand file: {:?}", e);
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

fn add_id_to_message(msg: OwnedMessage, ws_id: Arc<Mutex<u64>>) -> OwnedMessage {
    println!("MESAGE: {:?}", msg);
    if let OwnedMessage::Text(s) = &msg {
        let mut json: serde_json::Value = serde_json::from_str(s).unwrap();
        let id = *ws_id.lock().unwrap();
        *ws_id.lock().unwrap() += 1;
        json["id"] = serde_json::json!(id);
        OwnedMessage::Text(json.to_string())
    } else {
        unreachable!();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::notifier;
    use tokio::sync::mpsc;
    use websocket::OwnedMessage;

    #[test]
    fn check_add_message_id() {
        let ws_id = Arc::new(Mutex::new(100));
        let msg = OwnedMessage::Text("{\"TEST\":1}".to_string());
        let msg = super::add_id_to_message(msg, ws_id.clone());
        let json: serde_json::Value;
        if let OwnedMessage::Text(s) = msg {
            json = serde_json::from_str(&s).unwrap();
        } else {
            unreachable!();
        }

        let expected = "{\"id\":100,\"TEST\":1}";
        let expected: serde_json::Value = serde_json::from_str(expected).unwrap();

        assert_eq!(expected, json);
        assert_eq!(101, *ws_id.lock().unwrap());
    }

    // TODO
    #[test]
    fn check_simple_response() {
        let msg = OwnedMessage::Text("{\"id\":1,\"result\":{}}".to_string());
        let (tx, _) = mpsc::channel(1);
        let (tx2, _) = mpsc::channel(1);
        let scripts = Arc::new(Mutex::new(vec![]));
        let pending_breakpoints: Arc<Mutex<Vec<super::FileLocation>>> =
            Arc::new(Mutex::new(vec![]));
        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));

        super::analyse_message(msg, tx, tx2, scripts, pending_breakpoints, notifier);
    }

    #[test]
    fn check_internal_script_parsed() {
        let msg = OwnedMessage::Text("{\"method\":\"Debugger.scriptParsed\",\"params\":{\"scriptId\":\"7\",\"url\":\"internal/bootstrap/loaders.js\",\"startLine\":0,\"startColumn\":0,\"endLine\":312,\"endColumn\":0,\"executionContextId\":1,\"hash\":\"39ff95c38ab7c4bb459aabfe5c5eb3a27441a4d8\",\"executionContextAuxData\":{\"isDefault\":true},\"isLiveEdit\":false,\"sourceMapURL\":\"\",\"hasSourceURL\":false,\"isModule\":false,\"length\":9613}}".to_string());
        let (tx, _) = mpsc::channel(1);
        let (tx2, _) = mpsc::channel(1);
        let scripts = Arc::new(Mutex::new(vec![]));
        let pending_breakpoints: Arc<Mutex<Vec<super::FileLocation>>> =
            Arc::new(Mutex::new(vec![]));
        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));

        super::analyse_message(
            msg,
            tx.clone(),
            tx2.clone(),
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

        let msg = OwnedMessage::Text("{\"method\":\"Debugger.scriptParsed\",\"params\":{\"scriptId\":\"8\",\"url\":\"internal/bootstrap/node.js\",\"startLine\":0,\"startColumn\":0,\"endLine\":438,\"endColumn\":0,\"executionContextId\":1,\"hash\":\"3f184a9d8a71f2554b8b31895d935027129c91c4\",\"executionContextAuxData\":{\"isDefault\":true},\"isLiveEdit\":false,\"sourceMapURL\":\"\",\"hasSourceURL\":false,\"isModule\":false,\"length\":14904}}".to_string());

        super::analyse_message(msg, tx, tx2, scripts.clone(), pending_breakpoints, notifier);

        assert_eq!(scripts.clone().lock().unwrap().len(), 2);
        assert_eq!(
            scripts.clone().lock().unwrap()[1].file,
            "internal/bootstrap/node.js".to_string()
        );
        assert_eq!(
            scripts.clone().lock().unwrap()[1].script_id,
            "8".to_string()
        );
        assert_eq!(scripts.clone().lock().unwrap()[1].is_internal, false);
    }

    #[test]
    fn check_file_script_parsed() {
        let msg = OwnedMessage::Text("{\"method\":\"Debugger.scriptParsed\",\"params\":{\"scriptId\":\"52\",\"url\":\"file:///home/me/test.js\"}}".to_string());
        let (tx, _) = mpsc::channel(1);
        let (tx2, _) = mpsc::channel(1);
        let scripts = Arc::new(Mutex::new(vec![]));
        let pending_breakpoints: Arc<Mutex<Vec<super::FileLocation>>> =
            Arc::new(Mutex::new(vec![]));
        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));

        super::analyse_message(msg, tx, tx2, scripts.clone(), pending_breakpoints, notifier);

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
