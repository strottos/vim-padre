//! Node debugger

use std::io;
use std::process::{exit, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::debugger::Debugger;
use crate::notifier::Notifier;
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
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    node_process: Option<Command>,
    ws_tx: Arc<Mutex<Option<Sender<OwnedMessage>>>>,
    ws_id: Arc<Mutex<u64>>,
    pending_breakpoints: Vec<FileLocation>,
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
            pending_breakpoints: vec!(),
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

        let (otx, orx) = mpsc::channel(1);

        tokio::spawn(
            lines
                .for_each(move |line| {
                    eprintln!("{}", line);

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

                        let f = ClientBuilder::new(&uri)
                            .unwrap()
                            .async_connect_insecure()
                            .and_then(move |(duplex, _)| {
                                let (sink, stream) = duplex.split();

                                let tx_setup = tx.clone();

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
                                            "{\"method\":\"Runtime.runIfWaitingForDebugger\"}"
                                                .to_string(),
                                        ))
                                        .map(move |a| {
                                            println!("Sending setup: {:?}", a);
                                            tokio::spawn(
                                                otx.clone().send(true).map(|_| {}).map_err(|e| {
                                                    eprintln!("Error spawning node: {:?}", e);
                                                }),
                                            );
                                        })
                                        .map_err(|e| {
                                            println!("Error sending setup: {:?}", e);
                                        }),
                                );

                                stream
                                    .filter_map(|message| {
                                        println!("Message: {:?}", message);
                                        None
                                    })
                                    .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
                                    .map(move |msg| {
                                        if let OwnedMessage::Text(s) = &msg {
                                            let mut json: serde_json::Value =
                                                serde_json::from_str(s).unwrap();
                                            let id = *ws_id.lock().unwrap();
                                            *ws_id.lock().unwrap() += 1;
                                            json["id"] = serde_json::json!(id);
                                            println!("MESAGE: {:?}", json);
                                            OwnedMessage::Text(json.to_string())
                                        } else {
                                            unreachable!();
                                        }
                                    })
                                    .forward(sink)
                            })
                            .map(|_| ())
                            .map_err(|e| eprintln!("WebSocket err: {:?}", e));

                        tokio::spawn(f);
                    }

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

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
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

#[cfg(test)]
mod tests {}
