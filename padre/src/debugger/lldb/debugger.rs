//! lldb client debugger

use std::io;
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use bytes::Bytes;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    None,
    Listening,
    Working,
}

#[derive(Debug, Clone)]
pub enum ProcessStatus {
    None,
    // (PID)
    Running(u64),
    Paused,
}

#[derive(Debug, Clone)]
pub enum LLDBOutput {
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
    BreakpointMultiple,
    BreakpointPending,
    // (type, variable, value)
    PrintVariable(String, String, String),
    VariableNotFound,
}

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    lldb_status: Arc<Mutex<LLDBStatus>>,
    process_status: Arc<Mutex<ProcessStatus>>,
    lldb_in_tx: Option<Sender<Bytes>>,
    listener_tx: Arc<Mutex<Option<Sender<LLDBOutput>>>>,
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
            lldb_status: Arc::new(Mutex::new(LLDBStatus::None)),
            process_status: Arc::new(Mutex::new(ProcessStatus::None)),
            lldb_in_tx: None,
            listener_tx: Arc::new(Mutex::new(None)),
        }
    }

    fn check_path_exists(&self, path: &str) {
        if !Path::new(path).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", path),
            );
            println!("Can't spawn LLDB as {} does not exist", path);
            exit(1);
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {
        //self.check_path_exists(&self.debugger_cmd);
        self.check_path_exists(&self.run_cmd[0]);

        let (lldb_in_tx, lldb_in_rx) = mpsc::channel(1);
        let (lldb_out_tx, lldb_out_rx) = mpsc::channel(32);

        self.lldb_in_tx = Some(lldb_in_tx);

        let mut cmd = vec![self.debugger_cmd.clone(), "--".to_string()];
        cmd.extend(self.run_cmd.clone());
        spawn_process(cmd, lldb_in_rx, lldb_out_tx);

        let notifier = self.notifier.clone();
        let lldb_status = self.lldb_status.clone();
        let process_status = self.process_status.clone();
        let listener_tx = self.listener_tx.clone();
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        tokio::spawn(
            lldb_out_rx
                .for_each(move |output| {
                    let data = String::from_utf8_lossy(&output[..]);
                    let data = data.trim_matches(char::from(0));

                    lazy_static! {
                        static ref RE_PROCESS_STARTED: Regex =
                            Regex::new("^Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
                        static ref RE_PROCESS_EXITED: Regex =
                            Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
                        static ref RE_BREAKPOINT: Regex =
                            Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+):\\d+, address = 0x[0-9a-f]*$")
                                .unwrap();
                        static ref RE_BREAKPOINT_2: Regex =
                            Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+), address = 0x[0-9a-f]*$")
                                .unwrap();
                        static ref RE_BREAKPOINT_MULTIPLE: Regex =
                            Regex::new("Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
                        static ref RE_BREAKPOINT_PENDING: Regex =
                            Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$")
                                .unwrap();
                        static ref RE_STOPPED_AT_POSITION: Regex =
                            Regex::new(" *frame #\\d.*$").unwrap();
                        static ref RE_JUMP_TO_POSITION: Regex =
                            Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
                        static ref RE_PRINTED_VARIABLE: Regex =
                            Regex::new("^\\((.*)\\) ([\\S+]*) = .*$").unwrap();
                        static ref RE_PROCESS_NOT_RUNNING: Regex =
                            Regex::new("error: invalid process$").unwrap();
                        static ref RE_VARIABLE_NOT_FOUND: Regex =
                            Regex::new("error: no variable named '([^']*)' found in this frame$").unwrap();
                        static ref RE_PROCESS_RUNNING_WARNING: Regex =
                            Regex::new("There is a running process, kill it and restart\\?: \\[Y/n\\]")
                                .unwrap();
                    }

                    if data.contains("(lldb) ") {
                        // Check LLDB has started if we haven't already
                        let lldb_status_current = lldb_status.lock().unwrap().clone();
                        match lldb_status_current {
                            // LLDB Starting
                            LLDBStatus::None => {
                                // Send messages to LLDB for startup
                                tokio::spawn(
                                    lldb_in_tx
                                        .clone()
                                        .send(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                                );

                                tokio::spawn(
                                    lldb_in_tx
                                        .clone()
                                        .send(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                                );

                                tokio::spawn(
                                    lldb_in_tx
                                        .clone()
                                        .send(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                                );

                                *lldb_status.lock().unwrap() = LLDBStatus::Listening;
                                notifier.lock().unwrap().signal_started();
                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .clone()
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::LLDBStarted)
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                            LLDBStatus::Working => {
                                *lldb_status.lock().unwrap() = LLDBStatus::Listening;
                            }
                            LLDBStatus::Listening => {}
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
                            *process_status.lock().unwrap() = ProcessStatus::Running(pid);
                            if !listener_tx.lock().unwrap().is_none() {
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::ProcessLaunched(pid))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        for cap in RE_PROCESS_EXITED.captures_iter(line) {
                            let pid = cap[1].parse::<u64>().unwrap();
                            let exit_code = cap[2].parse::<u64>().unwrap();

                            *process_status.lock().unwrap() = ProcessStatus::None;
                            notifier.lock().unwrap().signal_exited(pid, exit_code);
                            if !listener_tx.lock().unwrap().is_none() {
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::ProcessExited(pid, exit_code))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
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
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::Breakpoint(file, line))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        if !found_breakpoint {
                            for cap in RE_BREAKPOINT_2.captures_iter(line) {
                                found_breakpoint = true;
                                let file = cap[2].to_string();
                                let line = cap[3].parse::<u64>().unwrap();
                                notifier.lock().unwrap().breakpoint_set(file.clone(), line);
                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::Breakpoint(file, line))
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        if !found_breakpoint {
                            for _ in RE_BREAKPOINT_MULTIPLE.captures_iter(line) {
                                found_breakpoint = true;
                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::BreakpointMultiple)
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        if !found_breakpoint {
                            for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::BreakpointPending)
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
                            *process_status.lock().unwrap() = ProcessStatus::Paused;

                            let mut found = false;
                            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                                found = true;
                                let file = cap[1].to_string();
                                let line = cap[2].parse::<u64>().unwrap();
                                notifier.lock().unwrap().jump_to_position(file.clone(), line);
                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::JumpToPosition(file, line))
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }

                            if !found {
                                notifier.lock().unwrap().log_msg(
                                    LogLevel::WARN, "Stopped at unknown position".to_string());

                                if !listener_tx.lock().unwrap().is_none() {
                                    tokio::spawn(
                                        listener_tx
                                            .lock()
                                            .unwrap()
                                            .take()
                                            .unwrap()
                                            .send(LLDBOutput::UnknownPosition)
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                    );
                                }
                            }
                        }

                        for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
                            let variable_type = cap[1].to_string();
                            let variable = cap[2].to_string();

                            let mut start = 1;

                            while &data[start..start+1] != ")" {
                                start += 1;
                            }
                            while &data[start..start+1] != "=" {
                                start += 1;
                            }
                            start += 2;

                            let mut end = data.len();

                            if data.contains("(lldb) ") {
                                end -= 9; // Strip off "\r\n(lldb) "
                            }

                            if !listener_tx.lock().unwrap().is_none() {
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::PrintVariable(
                                            variable_type,
                                            variable,
                                            data[start..end].to_string(),
                                        ))
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
                            notifier.lock().unwrap().log_msg(
                                LogLevel::WARN,
                                "program not running".to_string()
                            );

                            if !listener_tx.lock().unwrap().is_none() {
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::NoProcess)
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        for cap in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
                            let variable = cap[1].to_string();

                            notifier.lock().unwrap().log_msg(
                                LogLevel::WARN,
                                format!("variable '{}' doesn't exist here", variable)
                            );

                            if !listener_tx.lock().unwrap().is_none() {
                                tokio::spawn(
                                    listener_tx
                                        .lock()
                                        .unwrap()
                                        .take()
                                        .unwrap()
                                        .send(LLDBOutput::VariableNotFound)
                                        .map(|_| {})
                                        .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                                );
                            }
                        }

                        for _ in RE_PROCESS_RUNNING_WARNING.captures_iter(line) {
                            let lldb_status_current = lldb_status.lock().unwrap().clone();
                            match lldb_status_current {
                                LLDBStatus::Listening | LLDBStatus::Working => {
                                    tokio::spawn(
                                        lldb_in_tx
                                            .clone()
                                            .send(Bytes::from(&b"n\n"[..]))
                                            .map(|_| {})
                                            .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                                    );
                                }
                                _ => {}
                            }
                        //        tokio::spawn(
                        //            listener_tx
                        //                .lock()
                        //                .unwrap()
                        //                .take()
                        //                .unwrap()
                        //                .send(LLDBOutput::VariableNotFound)
                        //                .map(|_| {})
                        //                .map_err(|e| eprintln!("Error sending to analyser: {}", e))
                        //        );
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
        match *self.lldb_status.lock().unwrap() {
            LLDBStatus::None => false,
            _ => true,
        }
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "Launching process".to_string());

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let listener_tx = self.listener_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *listener_tx.lock().unwrap() = Some(tx);

        let stmt = format!("breakpoint set --name main\n");

        tokio::spawn(
            lldb_in_tx
                .clone()
                .send(Bytes::from(&stmt[..]))
                .timeout(Duration::new(5, 0))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .and_then(move |lldb_output| {
                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    LLDBOutput::Breakpoint(_, _) | LLDBOutput::BreakpointMultiple => {}
                    _ => {
                        panic!("WTF? {:?}", lldb_output);
                        // TODO: Error properly
                    }
                };

                Ok(())
            })
            .and_then(move |_| {
                let (tx, rx) = mpsc::channel(1);
                *listener_tx.lock().unwrap() = Some(tx);

                let stmt = format!("process launch\n");

                tokio::spawn(
                    lldb_in_tx
                        .send(Bytes::from(&stmt[..]))
                        .map(|_| {})
                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                );

                rx.take(1).into_future()
            })
            .timeout(Duration::new(10, 0))
            .map(|lldb_output| {
                let resp;

                let lldb_output = lldb_output.0.unwrap();
                match lldb_output {
                    LLDBOutput::ProcessLaunched(pid) => {
                        resp = serde_json::json!({"status":"OK","pid":format!("{}",pid)});
                    }
                    _ => {
                        panic!("WTF? {:?}", lldb_output);
                        // TODO: Error properly
                    }
                };

                resp
            })
            .map_err(|e| {
                eprintln!("Error sending to LLDB: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out spawning process")
            });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file: String,
        line: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        self.notifier.lock().unwrap().log_msg(
            LogLevel::INFO,
            format!(
                "Setting breakpoint in file {} at line number {}",
                file, line
            ),
        );

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let stmt = format!("breakpoint set --file {} --line {}\n", file, line);

        let listener_tx = self.listener_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *listener_tx.lock().unwrap() = Some(tx);

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .map(move |lldb_output| {
                let mut resp;

                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    LLDBOutput::Breakpoint(_, _) => {
                        resp = serde_json::json!({"status":"OK"});
                    }
                    LLDBOutput::BreakpointPending => {
                        resp = serde_json::json!({"status":"PENDING"});
                    }
                    _ => {
                        panic!("WTF? {:?}", lldb_output);
                        // TODO: Error properly
                    }
                };

                resp
            })
            .map_err(|e| {
                eprintln!("Error sending to LLDB: {}", e.0);
                io::Error::new(io::ErrorKind::Other, e.0)
            });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let stmt = "thread step-in\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let stmt = "thread step-over\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
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
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let stmt = "thread continue\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn print(
        &mut self,
        variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        let stmt = format!("frame variable {}\n", variable);

        let listener_tx = self.listener_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *listener_tx.lock().unwrap() = Some(tx);

        tokio::spawn(
            lldb_in_tx
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .map(move |lldb_output| {
                let mut resp;

                let lldb_output = lldb_output.0;

                match lldb_output {
                    Some(s) => match s {
                        LLDBOutput::PrintVariable(variable_type, variable, value) => {
                            resp = serde_json::json!({
                                "status": "OK",
                                "variable": variable,
                                "value": value,
                                "type": variable_type,
                            });
                        }
                        _ => {
                            resp = serde_json::json!({"status":"ERROR"});
                        }
                    },
                    _ => {
                        resp = serde_json::json!({"status":"ERROR"});
                    }
                };

                resp
            })
            .map_err(|e| {
                eprintln!("Error sending to LLDB: {}", e.0);
                io::Error::new(io::ErrorKind::Other, e.0)
            });

        Box::new(f)
    }
}
