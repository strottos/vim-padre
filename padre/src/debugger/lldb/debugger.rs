//! lldb client debugger

use std::io;
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};
use crate::request::RequestError;

use bytes::Bytes;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    NoProcess,
    Error,
    LLDBStarted,
    // (PID)
    ProcessLaunched(u64),
    // (PID, Exit code)
    ProcessExited(u64, u64),
    // (File name, line number)
    Breakpoint(String, u64),
    // (File name, line number)
    JumpToPosition(String, u64),
    UnknownPosition,
    BreakpointPending,
    StepIn,
    StepOver,
    Continue,
    Variable,
    VariableNotFound,
}

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    started: Arc<Mutex<bool>>,
    lldb_in_tx: Option<Sender<Bytes>>,
    listener_tx: Arc<Mutex<Option<Sender<LLDBStatus>>>>,
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
            started: Arc::new(Mutex::new(false)),
            lldb_in_tx: None,
            listener_tx: Arc::new(Mutex::new(None)),
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {
        if !Path::new(&self.run_cmd[0]).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]),
            );
            println!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]);
            exit(1);
        }

        let (lldb_in_tx, lldb_in_rx) = mpsc::channel(1);
        let (lldb_out_tx, lldb_out_rx) = mpsc::channel(32);

        self.lldb_in_tx = Some(lldb_in_tx);

        let mut cmd = vec![self.debugger_cmd.clone(), "--".to_string()];
        cmd.extend(self.run_cmd.clone());
        spawn_process(cmd, lldb_in_rx, lldb_out_tx);

        let notifier = self.notifier.clone();
        let started = self.started.clone();
        let listener_tx = self.listener_tx.clone();
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        tokio::spawn(
            lldb_out_rx
                .for_each(move |output| {
                    let data = String::from_utf8_lossy(&output[..]);
                    let data = data.trim_matches(char::from(0));

                    lazy_static! {
                        static ref RE_PROCESS_STARTED: Regex =
                            Regex::new("Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
                        static ref RE_PROCESS_EXITED: Regex =
                            Regex::new("Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
                        static ref RE_BREAKPOINT: Regex =
                            Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+):\\d+, address = 0x[0-9a-f]*$")
                                .unwrap();
                        static ref RE_BREAKPOINT_2: Regex =
                            Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+), address = 0x[0-9a-f]*$")
                                .unwrap();
                        static ref RE_BREAKPOINT_PENDING: Regex =
                            Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$")
                                .unwrap();
                        static ref RE_STOPPED_AT_POSITION: Regex =
                            Regex::new(" *frame #\\d.*$").unwrap();
                        static ref RE_JUMP_TO_POSITION: Regex =
                            Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
                    }

                    // Check LLDB has started
                    if *started.lock().unwrap() == false && data.contains("(lldb) ") {
                        // Send messages to LLDB for setup
                        tokio::spawn(
                            lldb_in_tx
                                .clone()
                                .send(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]))
                                .map(|_| {})
                                .map_err(|e| println!("Error sending to LLDB: {}", e)),
                        );

                        tokio::spawn(
                            lldb_in_tx
                                .clone()
                                .send(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]))
                                .map(|_| {})
                                .map_err(|e| println!("Error sending to LLDB: {}", e)),
                        );

                        tokio::spawn(
                            lldb_in_tx
                                .clone()
                                .send(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]))
                                .map(|_| {})
                                .map_err(|e| println!("Error sending to LLDB: {}", e)),
                        );

                        *started.lock().unwrap() = true;
                        notifier.lock().unwrap().signal_started();
                        if !listener_tx.lock().unwrap().is_none() {
                            println!("HERE1");
                            tokio::spawn(
                                listener_tx
                                    .clone()
                                    .lock()
                                    .unwrap()
                                    .take()
                                    .unwrap()
                                    .send(LLDBStatus::LLDBStarted)
                                    .map(|_| {})
                                    .map_err(|e| println!("Error sending to analyser: {}", e))
                            );
                        }
                    }

                    for line in data.split("\r\n") {
                        //        // TODO: Find a more efficient way of doing this, and maybe think about UTF-8
                        //        let mut line = line;
                        //        while line.len() > 7 && &line[0..7] == "(lldb) " {
                        //            line = &line[7..];
                        //        }

                        for cap in RE_PROCESS_STARTED.captures_iter(line) {
                            let pid = cap[1].parse::<u64>().unwrap();
                            println!("Process started {}", pid);
                            if !listener_tx.lock().unwrap().is_none() {
                                println!("HERE2");
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBStatus::ProcessLaunched(pid))
                                        .map(|_| {})
                                        .map_err(|e| println!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        for cap in RE_PROCESS_EXITED.captures_iter(line) {
                            let pid = cap[1].parse::<u64>().unwrap();
                            let exit_code = cap[2].parse::<u64>().unwrap();
                            notifier.lock().unwrap().signal_exited(pid, exit_code);
                            println!("Process exited {}", exit_code);
                            if !listener_tx.lock().unwrap().is_none() {
                                println!("HERE3");
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBStatus::ProcessExited(pid, exit_code))
                                        .map(|_| {})
                                        .map_err(|e| println!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        let mut found_breakpoint = false;

                        for cap in RE_BREAKPOINT.captures_iter(line) {
                            found_breakpoint = true;
                            let file = cap[2].to_string();
                            let line = cap[3].parse::<u64>().unwrap();
                            notifier.lock().unwrap().breakpoint_set(file.clone(), line);
                            if !listener_tx.lock().unwrap().is_none() {
                                println!("BREAK1");
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBStatus::Breakpoint(file, line))
                                        .map(|_| {})
                                        .map_err(|e| println!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        if !found_breakpoint {
                            for cap in RE_BREAKPOINT_2.captures_iter(line) {
                                found_breakpoint = true;
                                let file = cap[2].to_string();
                                let line = cap[3].parse::<u64>().unwrap();
                                notifier.lock().unwrap().breakpoint_set(file.clone(), line);
                                notifier.lock().unwrap().log_msg(
                                    LogLevel::INFO,
                                    format!(
                                        "Setting breakpoint in file {} at line number {}",
                                        file, line
                                    ),
                                );
                                if !listener_tx.lock().unwrap().is_none() {
                                    println!("BREAK2");
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBStatus::Breakpoint(file, line))
                                            .map(|_| {})
                                            .map_err(|e| println!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        if !found_breakpoint {
                            for cap in RE_BREAKPOINT_PENDING.captures_iter(line) {
                                if !listener_tx.lock().unwrap().is_none() {
                                    println!("BREAK3");
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBStatus::BreakpointPending)
                                            .map(|_| {})
                                            .map_err(|e| println!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
                            let mut found = false;
                            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                                found = true;
                                let file = cap[1].to_string();
                                let line = cap[2].parse::<u64>().unwrap();
                                notifier.lock().unwrap().jump_to_position(file.clone(), line);
                                if !listener_tx.lock().unwrap().is_none() {
                                    println!("HERE4");
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBStatus::JumpToPosition(file, line))
                                            .map(|_| {})
                                            .map_err(|e| println!("Error sending to analyser: {}", e))
                                    );
                                }
                            }

                            if !found {
                                if !listener_tx.lock().unwrap().is_none() {
                                    println!("HERE3");
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBStatus::UnknownPosition)
                                            .map(|_| {})
                                            .map_err(|e| println!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }
                    }

                    Ok(())
                })
                .map_err(|e| panic!("Error receiving from lldb: {}", e)),
        );

        // This is the preferred method but doesn't seem to work with current tokio
        // Example here states we need a separate thread: https://github.com/tokio-rs/tokio/blob/master/tokio/examples/connect.rs
        //
        //        let input = FramedRead::new(stdin(), LinesCodec::new());
        //
        //        tokio::spawn(
        //            input
        //                .for_each(|req| {
        //                    println!("{:?}", req);
        //                    Ok(())
        //                })
        //                .map(|_| ())
        //                .map_err(|e| panic!("io error = {:?}", e))
        //        );

        let mut lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        thread::spawn(|| {
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                let bytes = Bytes::from(buf);
                lldb_in_tx = match lldb_in_tx.send(bytes).wait() {
                    Ok(tx) => tx,
                    Err(_) => break,
                };
            }
        });
    }

    fn has_started(&self) -> bool {
        *self.started.lock().unwrap()
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "Launching process".to_string());

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let listener_tx = self.listener_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *listener_tx.lock().unwrap() = Some(tx);

        let breakpoint_set = format!("breakpoint set --name main\n");

        tokio::spawn(
            lldb_in_tx
                .clone()
                .send(Bytes::from(&breakpoint_set[..]))
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .and_then(move |lldb_status| {
                let lldb_status = lldb_status.0.unwrap();

                match lldb_status {
                    LLDBStatus::Breakpoint(_, _) => {}
                    _ => {
                        panic!("WTF? {:?}", lldb_status);
                        // TODO: Error properly
                    }
                };

                Ok(())
            })
            .and_then(move |_| {
                let (tx, rx) = mpsc::channel(1);
                *listener_tx.lock().unwrap() = Some(tx);

                let run = format!("process launch\n");

                tokio::spawn(
                    lldb_in_tx
                        .send(Bytes::from(&run[..]))
                        .map(|_| {})
                        .map_err(|e| println!("Error sending to LLDB: {}", e)),
                );

                rx.take(1).into_future()
            })
            .map(|lldb_status| {
                let mut resp;

                let lldb_status = lldb_status.0.unwrap();
                match lldb_status {
                    LLDBStatus::ProcessLaunched(pid) => {
                        resp = serde_json::json!({"status":"OK","pid":format!("{}",pid)});
                    }
                    _ => {
                        panic!("WTF? {:?}", lldb_status);
                        // TODO: Error properly
                    }
                };

                resp
            })
            .map_err(|e| {
                println!("Error sending to LLDB: {}", e.0);
                io::Error::new(io::ErrorKind::Other, e.0)
            });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file: String,
        line: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.notifier.lock().unwrap().log_msg(
            LogLevel::INFO,
            format!(
                "Setting breakpoint in file {} at line number {}",
                file, line
            ),
        );

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let breakpoint_set = format!("breakpoint set --file {} --line {}\n", file, line);

        let listener_tx = self.listener_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *listener_tx.lock().unwrap() = Some(tx);

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&breakpoint_set[..]))
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .map(move |lldb_status| {
                let mut resp;

                let lldb_status = lldb_status.0.unwrap();

                match lldb_status {
                    LLDBStatus::Breakpoint(_, _) => {
                        resp = serde_json::json!({"status":"OK"});
                    }
                    LLDBStatus::BreakpointPending => {
                        resp = serde_json::json!({"status":"PENDING"});
                    }
                    _ => {
                        panic!("WTF? {:?}", lldb_status);
                        // TODO: Error properly
                    }
                };

                resp
            })
            .map_err(|e| {
                println!("Error sending to LLDB: {}", e.0);
                io::Error::new(io::ErrorKind::Other, e.0)
            });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let step_in = "thread step-in\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&step_in[..]))
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let step_in = "thread step-over\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&step_in[..]))
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let step_in = "thread continue\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&step_in[..]))
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }
}
