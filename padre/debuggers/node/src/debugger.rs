//! Node debugger
//!
//! The main Node Debugger entry point. Handles spawning processes and communicating
//! with it through the websocket.

use std::io;
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::analyser::Analyser;
use super::process::Process;
use super::ws::WSHandler;
use padre_core::notifier::{log_msg, LogLevel};
use padre_core::server::{DebuggerV1, FileLocation, Variable};

use futures::prelude::*;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImplDebugger {
    process: Arc<Mutex<Process>>,
    ws_handler: Arc<Mutex<WSHandler>>,
    analyser: Arc<Mutex<Analyser>>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        let process = Arc::new(Mutex::new(Process::new(debugger_cmd, run_cmd)));
        let ws_handler = Arc::new(Mutex::new(WSHandler::new()));
        let analyser = Arc::new(Mutex::new(Analyser::new(ws_handler.clone())));
        ImplDebugger {
            process,
            ws_handler,
            analyser,
        }
    }
}

impl DebuggerV1 for ImplDebugger {
    fn setup(&mut self) {}

    fn teardown(&mut self) {
        exit(0);
    }

    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");

        let (tx, mut rx) = mpsc::channel(1);

        self.process.lock().unwrap().run(tx);

        let process = self.process.clone();
        let analyser = self.analyser.clone();
        let ws_handler = self.ws_handler.clone();

        tokio::spawn(async move {
            let uri = rx.next().await.unwrap();

            // We need a little sleep otherwise we fail to connect, shame to block
            // the thread but can live with it while starting up the process
            thread::sleep(Duration::new(2, 0));

            ws_handler.lock().unwrap().connect(&uri, move |msg| {
                analyser.lock().unwrap().analyse_message(msg);
                None
            });

            //let msg = OwnedMessage::Text("{\"method\":\"Runtime.enable\"}".to_string());
            //let resp1 = ws_handler
            //    .lock()
            //    .unwrap()
            //    .send_and_receive_message(msg)
            //    .await;
            //let msg = OwnedMessage::Text("{\"method\":\"Debugger.enable\"}".to_string());
            //let resp2 = ws_handler.lock().unwrap().send_and_receive_message(msg);
            //let msg = OwnedMessage::Text(
            //    "{\"method\":\"Runtime.runIfWaitingForDebugger\"}".to_string(),
            //);
            //let resp3 = ws_handler.lock().unwrap().send_and_receive_message(msg);

            //if !resp1["error"].is_null()
            //    || !resp2["error"].is_null()
            //    || !resp3["error"].is_null()
            //{
            //    serde_json::json!({"status":"ERROR"})
            //} else {
            //    let pid = process.lock().unwrap().get_pid();
            //    analyser.lock().unwrap().set_pid(pid);
            //    serde_json::json!({"status":"OK","pid":pid})
            //}
        });

        //let f = rx
        //    .take(1)
        //    .into_future()
        //    .and_then(move |uri| {
        //        // We need a little sleep otherwise we fail to connect,
        //        // shame to block the thread but can live with it while
        //        // starting up the process
        //        thread::sleep(Duration::new(2, 0));

        //        ws_handler
        //            .lock()
        //            .unwrap()
        //            .connect(&uri.0.unwrap(), move |msg| {
        //                analyser.lock().unwrap().analyse_message(msg);
        //                None
        //            });

        //        Ok(())
        //    })
        //    .then(move |_| {
        //        let msg = OwnedMessage::Text("{\"method\":\"Runtime.enable\"}".to_string());
        //        let f1 = ws_handler2
        //            .clone()
        //            .lock()
        //            .unwrap()
        //            .send_and_receive_message(msg);
        //        let msg = OwnedMessage::Text("{\"method\":\"Debugger.enable\"}".to_string());
        //        let f2 = ws_handler2.lock().unwrap().send_and_receive_message(msg);
        //        let msg = OwnedMessage::Text(
        //            "{\"method\":\"Runtime.runIfWaitingForDebugger\"}".to_string(),
        //        );
        //        let f3 = ws_handler2.lock().unwrap().send_and_receive_message(msg);

        //        f1.join(f2).join(f3)
        //    })
        //    .timeout(Duration::new(
        //        config
        //            .lock()
        //            .unwrap()
        //            .get_config("ProcessSpawnTimeout")
        //            .unwrap() as u64,
        //        0,
        //    ))
        //    .map(move |responses| {
        //        let resp1 = (responses.0).0;
        //        let resp2 = (responses.0).1;
        //        let resp3 = responses.1;
        //        if !resp1["error"].is_null()
        //            || !resp2["error"].is_null()
        //            || !resp3["error"].is_null()
        //        {
        //            serde_json::json!({"status":"ERROR"})
        //        } else {
        //            let pid = process.lock().unwrap().get_pid();
        //            analyser2.lock().unwrap().set_pid(pid);
        //            serde_json::json!({"status":"OK","pid":pid})
        //        }
        //    })
        //    .map_err(|e| {
        //        eprintln!("Reading stdin error {:?}", e);
        //        io::Error::new(io::ErrorKind::Other, "Timed out setting breakpoint")
        //    });
    }

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        let full_file_name = Path::new(&file_location.name()).canonicalize();
        let f = match full_file_name {
            Ok(s) => {
                let filename = s.to_string_lossy().to_string();
                let mut analyser = self.analyser.lock().unwrap();
                match analyser.get_script_from_filename(&filename) {
                    Some(script) => {
                        //let msg = OwnedMessage::Text(format!(
                        //    "{{\
                        //     \"method\":\"Debugger.setBreakpoint\",\
                        //     \"params\":{{\
                        //     \"location\":{{\
                        //     \"scriptId\":\"{}\",\
                        //     \"lineNumber\":{}\
                        //     }}\
                        //     }}\
                        //     }}",
                        //    script.get_script_id(),
                        //    file_location.line_num - 1
                        //));

                        //let line_num = file_location.line_num;

                        //self.ws_handler
                        //    .lock()
                        //    .unwrap()
                        //    .send_and_receive_message(msg)
                        //    .map(move |response| {
                        //        if response["error"].is_null() {
                        //            //breakpoint_set(&filename, line_num);

                        //            serde_json::json!({"status":"OK"})
                        //        } else {
                        //            serde_json::json!({"status":"ERROR"})
                        //        }
                        //    })
                    }
                    None => {
                        //analyser.add_pending_breakpoint(FileLocation::new(
                        //    filename,
                        //    file_location.line_num,
                        //));
                    }
                }
            }
            Err(e) => {
                log_msg(
                    LogLevel::ERROR,
                    &format!("Can't find file {}: {}", file_location.name(), e),
                );
            }
        };
    }

    fn unbreakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        //let msg = OwnedMessage::Text("{\"method\":\"Debugger.stepInto\"}".to_string());

        //let f = self
        //    .ws_handler
        //    .lock()
        //    .unwrap()
        //    .send_and_receive_message(msg)
        //    .map(|response| {
        //        if response["error"].is_null() {
        //            serde_json::json!({"status":"OK"})
        //        } else {
        //            serde_json::json!({"status":"ERROR"})
        //        }
        //    });
    }

    fn step_over(&mut self, _timeout: Instant) {
        //let msg = OwnedMessage::Text("{\"method\":\"Debugger.stepOver\"}".to_string());

        //let f = self
        //    .ws_handler
        //    .lock()
        //    .unwrap()
        //    .send_and_receive_message(msg)
        //    .map(|response| {
        //        if response["error"].is_null() {
        //            serde_json::json!({"status":"OK"})
        //        } else {
        //            serde_json::json!({"status":"ERROR"})
        //        }
        //    });
    }

    fn continue_(&mut self, _timeout: Instant) {
        //let msg = OwnedMessage::Text("{\"method\":\"Debugger.resume\"}".to_string());

        //let f = self
        //    .ws_handler
        //    .lock()
        //    .unwrap()
        //    .send_and_receive_message(msg)
        //    .map(|response| {
        //        if response["error"].is_null() {
        //            serde_json::json!({"status":"OK"})
        //        } else {
        //            serde_json::json!({"status":"ERROR"})
        //        }
        //    });
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        let variable = variable.name().clone();

        // let msg = OwnedMessage::Text(format!(
        //     "{{\
        //      \"method\":\"Debugger.evaluateOnCallFrame\",\
        //      \"params\":{{\
        //      \"callFrameId\":\"{{\\\"ordinal\\\":0,\\\"injectedScriptId\\\":1}}\",\
        //      \"expression\":\"{}\",\
        //      \"returnByValue\":true\
        //      }}\
        //      }}",
        //     variable,
        // ));

        //let f = self
        //    .ws_handler
        //    .lock()
        //    .unwrap()
        //    .send_and_receive_message(msg)
        //    .map(move |response| {
        //        if response["error"].is_null() {
        //            let mut json = response;
        //            let variable_type = json["result"]["result"]["type"].take();
        //            let value = json["result"]["result"]["value"].take();
        //            serde_json::json!({
        //                "status": "OK",
        //                "type": variable_type,
        //                "variable": variable,
        //                "value": value,
        //            })
        //        } else {
        //            serde_json::json!({"status":"ERROR"})
        //        }
        //    });
    }
}
