//! Node debugger
//!
//! The main Node Debugger entry point. Handles spawning processes and communicating
//! with it through the websocket.

use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::analyser::Analyser;
use super::process::Process;
use super::utils::get_json;
use super::ws::WSHandler;
use padre_core::debugger::{Debugger, DebuggerCmd, DebuggerCmdBasic, FileLocation, Variable};
use padre_core::notifier::{log_msg, LogLevel};

use futures::prelude::*;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug)]
pub struct ImplDebugger {
    debugger_cmd: String,
    run_cmd: Vec<String>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        ImplDebugger {
            debugger_cmd,
            run_cmd,
        }
    }
}

impl Debugger for ImplDebugger {
    /// Perform any initial setup including starting LLDB and setting up the stdio analyser stuff
    /// - startup lldb and setup the stdio analyser
    /// - perform initial setup so we can analyse LLDB properly
    #[allow(unreachable_patterns)]
    fn setup_handler(&self, mut queue_rx: mpsc::Receiver<(DebuggerCmd, Instant)>) {
        let debugger_cmd = self.debugger_cmd.clone();
        let run_cmd = self.run_cmd.clone();

        tokio::spawn(async move {
            let mut debugger = NodeDebugger::new(debugger_cmd, run_cmd);

            while let Some(cmd) = queue_rx.next().await {
                match cmd.0 {
                    DebuggerCmd::Basic(basic_cmd) => match basic_cmd {
                        DebuggerCmdBasic::Run => debugger.run(cmd.1),
                        DebuggerCmdBasic::Interrupt => debugger.interrupt(),
                        DebuggerCmdBasic::Exit => {
                            debugger.teardown();
                            break;
                        }
                        DebuggerCmdBasic::Breakpoint(fl) => debugger.breakpoint(&fl, cmd.1),
                        DebuggerCmdBasic::Unbreakpoint(fl) => debugger.unbreakpoint(&fl, cmd.1),
                        DebuggerCmdBasic::StepIn => debugger.step_in(cmd.1),
                        DebuggerCmdBasic::StepOver => debugger.step_over(cmd.1),
                        DebuggerCmdBasic::Continue => debugger.continue_(cmd.1),
                        DebuggerCmdBasic::Print(v) => debugger.print(&v, cmd.1),
                    },
                    _ => {
                        log_msg(LogLevel::WARN, "Got a command that wasn't understood");
                    }
                };
            }

            exit(0);
        });
    }
}

#[derive(Debug)]
pub struct NodeDebugger {
    debugger_cmd: String,
    run_cmd: Vec<String>,
    process: Option<Process>,
    // Used to send a message to node, can also be sent an optional Sender for the response
    node_tx: mpsc::Sender<(Message, Option<oneshot::Sender<Message>>)>,
    analyser: Arc<Mutex<Analyser>>,
}

impl NodeDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        // Hack, will be replaced after run.
        let (node_tx, _) = mpsc::channel(1);

        NodeDebugger {
            debugger_cmd,
            run_cmd,
            process: None,
            node_tx,
            analyser: Arc::new(Mutex::new(Analyser::new())),
        }
    }

    fn interrupt(&mut self) {}

    fn teardown(&mut self) {
        match self.process.take() {
            Some(mut p) => {
                p.stop();
            }
            None => {}
        };
    }

    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");

        let (awakener_tx, mut awakener_rx) = mpsc::channel(32);

        let process = Process::new(self.debugger_cmd.clone(), self.run_cmd.clone(), awakener_tx);

        let pid = process.get_pid();
        self.analyser.lock().unwrap().set_pid(pid);

        self.process = Some(process);

        let (node_tx, mut node_rx) = mpsc::channel(32);

        self.node_tx = node_tx.clone();

        let analyser = self.analyser.clone();

        tokio::spawn(async move {
            let uri = awakener_rx.next().await.unwrap();

            // We need a little sleep otherwise we fail to connect, shame to block
            // the thread but can live with it while starting up the process
            thread::sleep(Duration::new(2, 0));

            let mut ws_handler = WSHandler::new(&uri, analyser.clone(), node_tx.clone());

            let message = Message::Text(r#"{"method":"Runtime.enable"}"#.to_string());
            let resp1 = ws_handler.send_and_receive_message(message).await;
            let resp1 = get_json(&resp1);

            let message = Message::Text(r#"{"method":"Debugger.enable"}"#.to_string());
            let resp2 = ws_handler.send_and_receive_message(message).await;
            let resp2 = get_json(&resp2);

            let message =
                Message::Text(r#"{"method":"Runtime.runIfWaitingForDebugger"}"#.to_string());
            let resp3 = ws_handler.send_and_receive_message(message).await;
            let resp3 = get_json(&resp3);

            if !resp1.get("error").is_none()
                || !resp2.get("error").is_none()
                || !resp3.get("error").is_none()
            {
                log_msg(
                    LogLevel::ERROR,
                    &format!(
                        "Can't connect to node debugger, responses: {:?} {:?} {:?}",
                        resp1, resp2, resp3
                    ),
                );
            } else {
                log_msg(
                    LogLevel::INFO,
                    &format!(
                        "Node launched with pid {} and PADRE connected to debugger",
                        pid
                    ),
                );
            }

            tokio::spawn(async move {
                let mut ws_handler = ws_handler;
                while let Some((message, sender)) = node_rx.next().await {
                    let response = ws_handler.send_and_receive_message(message).await;
                    match sender {
                        Some(tx) => {
                            tx.send(response).unwrap();
                        }
                        None => {}
                    };
                }
            });
        });
    }

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        let full_file_name = Path::new(&file_location.name()).canonicalize();
        let mut node_tx = self.node_tx.clone();

        match full_file_name {
            Ok(s) => {
                let filename = s.to_string_lossy().to_string();
                let line_num = file_location.line_num();

                let mut analyser = self.analyser.lock().unwrap();
                match analyser.get_script_from_filename(&filename) {
                    Some(script) => {
                        let message = Message::Text(format!(
                            "{{\
                             \"method\":\"Debugger.setBreakpoint\",\
                             \"params\":{{\
                             \"location\":{{\
                             \"scriptId\":\"{}\",\
                             \"lineNumber\":{}\
                             }}\
                             }}\
                             }}",
                            script.get_script_id(),
                            line_num - 1
                        ));

                        tokio::spawn(async move {
                            let (tx, rx) = oneshot::channel();
                            node_tx.send((message, Some(tx))).await.unwrap();
                            let response = rx.await.unwrap();
                            let response = get_json(&response);
                            log_msg(
                                LogLevel::INFO,
                                &format!(
                                    "Breakpoint set at file {} and line number {}",
                                    &filename,
                                    response["result"]["actualLocation"]["lineNumber"]
                                        .as_u64()
                                        .unwrap()
                                        + 1
                                ),
                            )
                        });
                    }
                    None => {
                        log_msg(
                            LogLevel::INFO,
                            &format!(
                                "Breakpoint pending in file {} at line number {}",
                                filename, line_num
                            ),
                        );
                        analyser.add_pending_breakpoint(FileLocation::new(filename, line_num));
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

    fn unbreakpoint(&mut self, _file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        let mut node_tx = self.node_tx.clone();

        tokio::spawn(async move {
            let message = Message::Text(r#"{"method":"Debugger.stepInto"}"#.to_string());
            node_tx.send((message, None)).await.unwrap();
        });
    }

    fn step_over(&mut self, _timeout: Instant) {
        let mut node_tx = self.node_tx.clone();

        tokio::spawn(async move {
            let message = Message::Text(r#"{"method":"Debugger.stepOver"}"#.to_string());
            node_tx.send((message, None)).await.unwrap();
        });
    }

    fn continue_(&mut self, _timeout: Instant) {
        let mut node_tx = self.node_tx.clone();

        tokio::spawn(async move {
            let message = Message::Text(r#"{"method":"Debugger.resume"}"#.to_string());
            node_tx.send((message, None)).await.unwrap();
        });
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        let mut node_tx = self.node_tx.clone();

        let variable_name = variable.name().to_string();

        let message = Message::Text(format!(
            "{{\
             \"method\":\"Debugger.evaluateOnCallFrame\",\
             \"params\":{{\
             \"callFrameId\":\"{{\\\"ordinal\\\":0,\\\"injectedScriptId\\\":1}}\",\
             \"expression\":\"{}\",\
             \"returnByValue\":true\
             }}\
             }}",
            variable_name,
        ));

        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            node_tx.send((message, Some(tx))).await.unwrap();
            let response = rx.await.unwrap();
            let mut response = get_json(&response);
            let variable_type = response["result"]["result"]["type"]
                .take()
                .as_str()
                .unwrap()
                .to_string();
            let value = response["result"]["result"]["value"].take();
            log_msg(
                LogLevel::INFO,
                &format!("({}) {}={}", variable_type, variable_name, value),
            )
        });
    }
}
