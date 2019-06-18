//! lldb client debugger

use std::io;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};
use crate::util::{file_exists, get_file_full_path};

use bytes::Bytes;
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    None,
    Listening,
    Working,
    Quitting,
}

#[derive(Debug, Clone)]
pub enum ProcessStatus {
    None,
    Running,
    Paused,
}

#[derive(Debug, Clone)]
pub enum LLDBOutput {
    NoProcess,
    LLDBStarted,
    // (PID)
    ProcessLaunched(Pid),
    // (PID, Exit code)
    ProcessExited(Pid, i64),
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
    lldb_handler: Arc<Mutex<LLDBHandler>>,
}

#[derive(Debug)]
struct LLDBHandler {
    notifier: Arc<Mutex<Notifier>>,
    lldb_status: Arc<Mutex<LLDBStatus>>,
    lldb_pid: Option<Pid>,
    process_status: Arc<Mutex<ProcessStatus>>,
    process_pid: Arc<Mutex<Option<Pid>>>,
    lldb_in_tx: Option<Sender<Bytes>>,
    listener_tx: Arc<Mutex<Option<Sender<LLDBOutput>>>>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        let lldb_handler = Arc::new(Mutex::new(LLDBHandler::new(notifier.clone())));
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            lldb_handler,
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {
        let mut not_found = None;

        // Try getting the full path if the debugger doesn't exist
        if !file_exists(&self.debugger_cmd) {
            self.debugger_cmd = get_file_full_path(&self.debugger_cmd);
        }

        // Now check the debugger and program to debug exist, if not error
        if !file_exists(&self.debugger_cmd) {
            not_found = Some(&self.debugger_cmd);
        }

        if !file_exists(&self.run_cmd[0]) {
            not_found = Some(&self.run_cmd[0]);
        };

        if let Some(s) = not_found {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", s),
            );
            println!("Can't spawn LLDB as {} does not exist", s);

            exit(1);
        }

        // Stdin and Stdout/Stderr of LLDB
        let (lldb_in_tx, lldb_in_rx) = mpsc::channel(1);
        let (lldb_out_tx, lldb_out_rx) = mpsc::channel(32);

        self.lldb_handler.lock().unwrap().lldb_in_tx = Some(lldb_in_tx);

        let mut cmd = vec![self.debugger_cmd.clone(), "--".to_string()];
        cmd.extend(self.run_cmd.clone());
        self.lldb_handler.lock().unwrap().lldb_pid = Some(spawn_process(cmd, lldb_in_rx, lldb_out_tx));

        let lldb_handler = self.lldb_handler.clone();

        tokio::spawn(
            lldb_out_rx
                .for_each(move |output| {
                    let data = String::from_utf8_lossy(&output[..]);
                    let data = data.trim_matches(char::from(0));
                    lldb_handler.lock().unwrap().analyse_lldb_output(data);

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

        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        thread::spawn(move || {
            let mut lldb_in_tx = lldb_in_tx.clone().unwrap();
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

    fn teardown(&mut self) {
        match *self.lldb_handler.lock().unwrap().process_pid.lock().unwrap() {
            Some(pid) => match kill(pid, Signal::SIGINT) {
                Ok(_) => {}
                Err(e) => {
                    if e.as_errno().unwrap() != Errno::ESRCH {
                        panic!("Can't kill process: {}", e);
                    }
                }
            },
            None => {}
        }

        let current_status = self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap().clone();

        match current_status {
            LLDBStatus::None => return,
            LLDBStatus::Listening => {
                match self.lldb_handler.lock().unwrap().lldb_pid {
                    Some(pid) => {
                        kill(pid, Signal::SIGTERM).unwrap();
                        sleep(Duration::new(1, 0));
                        kill(pid, Signal::SIGKILL).unwrap();
                    }
                    None => (),
                }
                *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Quitting;
            }
            LLDBStatus::Working => {
                *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Quitting;
            }
            LLDBStatus::Quitting => {}
        }

        exit(0);

        // TODO: Preferred method
        //
        //let lldb_in_tx = self.lldb_in_tx.clone().unwrap();
        //
        //let stmt = format!("quit\n");
        //
        //tokio::spawn(
        //    lldb_in_tx
        //        .clone()
        //        .send(Bytes::from(&stmt[..]))
        //        .map(|_| {})
        //        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        //);
    }

    fn has_started(&self) -> bool {
        match *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() {
            LLDBStatus::None => false,
            _ => true,
        }
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "Launching process".to_string());

        let lldb_handler = self.lldb_handler.clone();
        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *lldb_handler.lock().unwrap().listener_tx.lock().unwrap() = Some(tx);

        let stmt = format!("breakpoint set --name main\n");

        tokio::spawn(
            lldb_in_tx
                .unwrap()
                .send(Bytes::from(&stmt[..]))
                .timeout(Duration::new(5, 0))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let f = rx
            .take(1)
            .into_future()
            .and_then(move |lldb_output| {
                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    LLDBOutput::Breakpoint(_, _) | LLDBOutput::BreakpointMultiple => {}
                    _ => {
                        panic!("Don't understand output {:?}", lldb_output);
                    }
                };

                Ok(())
            })
            .and_then(move |_| {
                let (tx, rx) = mpsc::channel(1);
                *lldb_handler.clone().lock().unwrap().listener_tx.lock().unwrap() = Some(tx);

                let stmt = format!("process launch\n");

                tokio::spawn(
                    lldb_in_tx
                        .unwrap()
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
                        panic!("Don't understand output {:?}", lldb_output);
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
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        self.notifier.lock().unwrap().log_msg(
            LogLevel::INFO,
            format!(
                "Setting breakpoint in file {} at line number {}",
                file, line
            ),
        );

        let lldb_handler = self.lldb_handler.clone();
        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *lldb_handler.lock().unwrap().listener_tx.lock().unwrap() = Some(tx);

        let stmt = format!("breakpoint set --file {} --line {}\n", file, line);

        tokio::spawn(
            lldb_in_tx
                .unwrap()
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .timeout(Duration::new(2, 0))
            .map(move |lldb_output| {
                let resp;

                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    LLDBOutput::Breakpoint(_, _) => {
                        resp = serde_json::json!({"status":"OK"});
                    }
                    LLDBOutput::BreakpointPending => {
                        resp = serde_json::json!({"status":"PENDING"});
                    }
                    _ => {
                        panic!("Don't understand output {:?}", lldb_output);
                    }
                };

                resp
            })
            .map_err(|e| {
                eprintln!("Error sending to LLDB: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out setting breakpoint")
            });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let stmt = "thread step-in\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .unwrap()
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
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let stmt = "thread step-over\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .unwrap()
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
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let stmt = "thread continue\n".to_string();

        tokio::spawn(
            lldb_in_tx
                .unwrap()
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
        *self.lldb_handler.lock().unwrap().lldb_status.lock().unwrap() = LLDBStatus::Working;

        let lldb_handler = self.lldb_handler.clone();
        let lldb_in_tx = self.lldb_handler.lock().unwrap().lldb_in_tx.clone();

        let (tx, rx) = mpsc::channel(1);
        *lldb_handler.lock().unwrap().listener_tx.lock().unwrap() = Some(tx);

        let stmt = format!("frame variable {}\n", variable);

        tokio::spawn(
            lldb_in_tx
                .unwrap()
                .send(Bytes::from(&stmt[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        let f = rx
            .take(1)
            .into_future()
            .timeout(Duration::new(10, 0))
            .map(move |lldb_output| {
                let mut resp = serde_json::json!({"status":"ERROR"});

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
                        _ => {}
                    },
                    _ => {}
                };

                resp
            })
            .map_err(|e| {
                eprintln!("Error sending to LLDB: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out printing variable")
            });

        Box::new(f)
    }
}

impl LLDBHandler {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
    ) -> LLDBHandler {
        LLDBHandler {
            notifier,
            lldb_in_tx: None,
            lldb_pid: None,
            lldb_status: Arc::new(Mutex::new(LLDBStatus::None)),
            process_pid: Arc::new(Mutex::new(None)),
            process_status: Arc::new(Mutex::new(ProcessStatus::None)),
            listener_tx: Arc::new(Mutex::new(None)),
        }
    }

    fn analyse_lldb_output(
        &self,
        data: &str,
    ) {
        lazy_static! {
            static ref RE_PROCESS_STARTED: Regex =
                Regex::new("^Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
            static ref RE_PROCESS_EXITED: Regex = Regex::new(
                "^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$"
            )
            .unwrap();
            static ref RE_BREAKPOINT: Regex = Regex::new(
                "Breakpoint (\\d+): where = .* at (.*):(\\d+):\\d+, address = 0x[0-9a-f]*$"
            )
            .unwrap();
            static ref RE_BREAKPOINT_2: Regex = Regex::new(
                "Breakpoint (\\d+): where = .* at (.*):(\\d+), address = 0x[0-9a-f]*$"
            )
            .unwrap();
            static ref RE_BREAKPOINT_MULTIPLE: Regex =
                Regex::new("Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
            static ref RE_BREAKPOINT_PENDING: Regex =
                Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
            static ref RE_STOPPED_AT_POSITION: Regex =
                Regex::new(" *frame #\\d.*$").unwrap();
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
            static ref RE_PRINTED_VARIABLE: Regex =
                Regex::new("^\\((.*)\\) ([\\S+]*) = .*$").unwrap();
            static ref RE_PROCESS_NOT_RUNNING: Regex =
                Regex::new("error: invalid process$").unwrap();
            static ref RE_VARIABLE_NOT_FOUND: Regex =
                Regex::new("error: no variable named '([^']*)' found in this frame$")
                    .unwrap();
            static ref RE_PROCESS_RUNNING_WARNING: Regex =
                Regex::new("There is a running process, kill it and restart\\?: \\[Y/n\\]")
                    .unwrap();
        }

        if data.contains("(lldb) ") {
            // Check LLDB has started if we haven't already
            let current_status = self.lldb_status.lock().unwrap().clone();
            match current_status {
                // LLDB Starting
                LLDBStatus::None => {
                    self.lldb_startup();
                }
                LLDBStatus::Working => {
                    *self.lldb_status.lock().unwrap() = LLDBStatus::Listening;
                }
                LLDBStatus::Listening | LLDBStatus::Quitting => {}
            }
        }

        for line in data.split("\r\n") {
            for cap in RE_PROCESS_STARTED.captures_iter(line) {
                let pid = Pid::from_raw(cap[1].parse::<i32>().unwrap());
                self.process_started(pid);
            }

            for cap in RE_PROCESS_EXITED.captures_iter(line) {
                let pid = Pid::from_raw(cap[1].parse::<i32>().unwrap());
                let exit_code = cap[2].parse::<i64>().unwrap();
                self.process_exited(pid, exit_code);
            }

            let mut found_breakpoint = false;

            for cap in RE_BREAKPOINT.captures_iter(line) {
                found_breakpoint = true;
                let file = cap[2].to_string();
                let line = cap[3].parse::<u64>().unwrap();
                self.found_breakpoint(file, line);
            }

            if !found_breakpoint {
                for cap in RE_BREAKPOINT_2.captures_iter(line) {
                    found_breakpoint = true;
                    let file = cap[2].to_string();
                    let line = cap[3].parse::<u64>().unwrap();
                    self.found_breakpoint(file, line);
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_MULTIPLE.captures_iter(line) {
                    found_breakpoint = true;
                    self.found_multiple_breakpoints();
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
                    self.found_pending_breakpoint();
                }
            }

            for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
                *self.process_status.lock().unwrap() = ProcessStatus::Paused;

                let mut found = false;
                for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                    found = true;
                    let file = cap[1].to_string();
                    let line = cap[2].parse::<u64>().unwrap();
                    self.jump_to_position(file, line);
                }

                if !found {
                    self.jump_to_unknown_position();
                }
            }

            for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
                let variable_type = cap[1].to_string();
                let variable = cap[2].to_string();
                self.printed_variable(variable_type, variable, data);
            }

            for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
                self.process_not_running();
            }

            for cap in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
                let variable = cap[1].to_string();
                self.variable_not_found(&variable);
            }

            for _ in RE_PROCESS_RUNNING_WARNING.captures_iter(line) {
                self.process_running_warning();
            }
        }
    }

    // Setup LLDB when in startup
    fn lldb_startup(&self) {
        let lldb_in_tx = self.lldb_in_tx.clone();

        // Send messages to LLDB for startup
        tokio::spawn(
            lldb_in_tx
                .clone()
                .unwrap()
                .send(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        tokio::spawn(
            lldb_in_tx
                .clone()
                .unwrap()
                .send(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        tokio::spawn(
            lldb_in_tx
                .unwrap()
                .send(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]))
                .map(|_| {})
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );

        *self.lldb_status.lock().unwrap() = LLDBStatus::Listening;
        self.notifier.lock().unwrap().signal_started();
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::LLDBStarted)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn process_started(&self, pid: Pid) {
        *self.process_status.lock().unwrap() = ProcessStatus::Running;
        *self.process_pid.lock().unwrap() = Some(pid);
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::ProcessLaunched(pid))
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn process_exited(&self, pid: Pid, exit_code: i64) {
        *self.process_status.lock().unwrap() = ProcessStatus::None;
        *self.process_pid.lock().unwrap() = None;
        self.notifier
            .lock()
            .unwrap()
            .signal_exited(pid.as_raw() as u64, exit_code);
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::ProcessExited(pid, exit_code))
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn found_breakpoint(&self, file: String, line: u64) {
        self.notifier.lock().unwrap().breakpoint_set(file.clone(), line);
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::Breakpoint(file, line))
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn found_multiple_breakpoints(&self) {
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::BreakpointMultiple)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn found_pending_breakpoint(&self) {
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::BreakpointPending)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }


    fn jump_to_position(&self, file: String, line: u64) {
        self.notifier
            .lock()
            .unwrap()
            .jump_to_position(file.clone(), line);
        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::JumpToPosition(file, line))
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn jump_to_unknown_position(&self) {
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::WARN, "Stopped at unknown position".to_string());

        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::UnknownPosition)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn printed_variable(&self, variable_type: String, variable: String, data: &str) {
        let mut start = 1;

        while &data[start..start + 1] != ")" {
            start += 1;
        }
        while &data[start..start + 1] != "=" {
            start += 1;
        }
        start += 2;

        let mut end = data.len();

        if data.contains("(lldb) ") {
            end -= 9; // Strip off "\r\n(lldb) "
        }

        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::PrintVariable(
                        variable_type,
                        variable,
                        data[start..end].to_string(),
                    ))
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn process_not_running(&self) {
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::WARN, "program not running".to_string());

        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::NoProcess)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn variable_not_found(&self, variable: &str) {
        self.notifier.lock().unwrap().log_msg(
            LogLevel::WARN,
            format!("variable '{}' doesn't exist here", variable),
        );

        if !self.listener_tx.lock().unwrap().is_none() {
            let listener_tx = self.listener_tx.clone();
            tokio::spawn(
                listener_tx
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap()
                    .send(LLDBOutput::VariableNotFound)
                    .map(|_| {})
                    .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
            );
        }
    }

    fn process_running_warning(&self) {
        let lldb_status_current = self.lldb_status.lock().unwrap().clone();
        let lldb_in_tx = self.lldb_in_tx.clone();
        match lldb_status_current {
            LLDBStatus::Listening | LLDBStatus::Working => {
                tokio::spawn(
                    lldb_in_tx
                        .unwrap()
                        .send(Bytes::from(&b"n\n"[..]))
                        .map(|_| {})
                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                );
            }
            LLDBStatus::Quitting => {
                tokio::spawn(
                    lldb_in_tx
                        .unwrap()
                        .send(Bytes::from(&b"Y\n"[..]))
                        .map(|_| {})
                        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
                );
            }
            _ => {}
        }
    }
}
