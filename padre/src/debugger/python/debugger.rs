//! Python debugger

use std::collections::HashMap;
use std::io;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use bytes::Bytes;
use nix::unistd::Pid;
use nix::sys::signal::{kill, Signal};
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug)]
struct FileLocation {
    file: String,
    line_num: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PDBStatus {
    None,
    Running,
    ReadyToPrint(String),
    Printing(String),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PDBInput {
    // (File name, line number)
    Breakpoint(String, u64),
    // (variable)
    PrintVariable(String),
}

#[derive(Debug, Clone)]
pub enum PDBOutput {
    None,
    // (File name, line number)
    Breakpoint(String, u64),
    // (value)
    PrintVariable(String),
}

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    pdb_handler: Arc<Mutex<PdbHandler>>,
    pending_breakpoints: Arc<Mutex<Vec<FileLocation>>>,
}

#[derive(Debug)]
struct PdbHandler {
    notifier: Arc<Mutex<Notifier>>,
    pid: Option<Pid>,
    in_tx: Option<Sender<Bytes>>,
    listener_senders: HashMap<PDBInput, Sender<PDBOutput>>,
    status: PDBStatus,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        let pdb_handler = Arc::new(Mutex::new(PdbHandler::new(notifier.clone())));
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            pdb_handler,
            pending_breakpoints: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {}

    fn teardown(&mut self) {
        let pid = self.pdb_handler.lock().unwrap().pid.unwrap();

        kill(pid, Signal::SIGTERM).unwrap();
        sleep(Duration::new(1, 0));
        kill(pid, Signal::SIGKILL).unwrap();

        exit(0);
    }

    fn has_started(&self) -> bool {
        true
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.pdb_handler.lock().unwrap().status = PDBStatus::Running;

        // TODO: Check debugger and program exist

        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "Launching process".to_string());

        // Stdin and Stdout/Stderr of Python
        let (py_in_tx, py_in_rx) = mpsc::channel(1);
        let (py_out_tx, py_out_rx) = mpsc::channel(1);

        self.pdb_handler.lock().unwrap().in_tx = Some(py_in_tx);

        let mut cmd = vec![
            self.debugger_cmd.clone(),
            "-m".to_string(),
            "pdb".to_string(),
            "--".to_string(),
        ];
        cmd.extend(self.run_cmd.clone());

        self.pdb_handler.lock().unwrap().pid = Some(spawn_process(cmd, py_in_rx, py_out_tx));

        let pdb_handler = self.pdb_handler.clone();

        tokio::spawn(
            py_out_rx
                .for_each(move |output| {
                    let data = String::from_utf8_lossy(&output[..]);
                    let data = data.trim_matches(char::from(0));
                    pdb_handler.lock().unwrap().analyse_output(data);

                    Ok(())
                })
                .map_err(|e| panic!("Error receiving from pdb: {}", e)),
        );

        let py_in_tx = self.pdb_handler.lock().unwrap().in_tx.clone();

        // Example here states we need a separate thread: https://github.com/tokio-rs/tokio/blob/master/tokio/examples/connect.rs
        thread::spawn(move || {
            let mut py_in_tx = py_in_tx.clone().unwrap();
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                let bytes = Bytes::from(buf);
                py_in_tx = match py_in_tx.send(bytes).wait() {
                    Ok(tx) => tx,
                    Err(_) => break,
                };
            }
        });

        let pdb_handler = self.pdb_handler.clone();

        for bkpt in self.pending_breakpoints.lock().unwrap().iter() {
            tokio::spawn(
                pdb_handler
                    .clone()
                    .lock()
                    .unwrap()
                    .send_and_receive(PDBInput::Breakpoint(bkpt.file.clone(), bkpt.line_num))
                    .map(|_| {})
                    .map_err(|e| {
                        eprintln!("Error sending to pdb: {}", e);
                    }),
            );
        }

        let pid = self.pdb_handler.lock().unwrap().pid.unwrap();

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK","pid":format!("{}",pid)});
            Ok(resp)
        });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file: String,
        line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.notifier.lock().unwrap().log_msg(
            LogLevel::INFO,
            format!(
                "Setting breakpoint in file {} at line number {}",
                file, line_num
            ),
        );

        let pdb_handler = self.pdb_handler.clone();
        let pending_breakpoints = self.pending_breakpoints.clone();

        Box::new(
            pdb_handler
                .clone()
                .lock()
                .unwrap()
                .send_and_receive(PDBInput::Breakpoint(file.clone(), line_num))
                .map(move |output| match output {
                    PDBOutput::Breakpoint(_, _) => serde_json::json!({"status":"OK"}),
                    PDBOutput::None => {
                        pending_breakpoints
                            .lock()
                            .unwrap()
                            .push(FileLocation { file, line_num });
                        serde_json::json!({"status":"PENDING"})
                    }
                    _ => {
                        unreachable!();
                    }
                }),
        )
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.pdb_handler.lock().unwrap().send("step");

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.pdb_handler.lock().unwrap().send("next");

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.pdb_handler.lock().unwrap().send("continue");

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
        self.pdb_handler.lock().unwrap().status = PDBStatus::ReadyToPrint(variable.to_string());

        let variable = variable.to_string();

        let pdb_handler = self.pdb_handler.clone();

        let f = pdb_handler
            .clone()
            .lock()
            .unwrap()
            .send_and_receive(PDBInput::PrintVariable(variable.clone()));

        Box::new(
            f.map(move |value| {
                match value {
                    PDBOutput::PrintVariable(value) => serde_json::json!({
                        "status": "OK",
                        "variable": variable,
                        "value": value,
                    }),
                    _ => {
                        unreachable!();
                    }
                }
            })
        )
    }
}

impl PdbHandler {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> PdbHandler {
        PdbHandler {
            notifier,
            pid: None,
            in_tx: None,
            listener_senders: HashMap::new(),
            status: PDBStatus::None,
        }
    }

    fn analyse_output(&mut self, data: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex =
                Regex::new("^Breakpoint (\\d*) at (.*):(\\d*)$").unwrap();
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)$").unwrap();
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("^The program finished and will be restarted$").unwrap();
            static ref RE_PROCESS_EXITED_WITH_CODE: Regex =
                Regex::new("^The program exited via sys.exit\\(\\)\\. Exit status: (-?\\d*)$")
                    .unwrap();
        }

        for line in data.split("\r\n") {
            if line.contains("(Pdb) ") {
                match self.status {
                    PDBStatus::Printing(_) => self.status = PDBStatus::Running,
                    _ => {}
                };
            }

            for cap in RE_BREAKPOINT.captures_iter(line) {
                let file = cap[2].to_string();
                let line = cap[3].parse::<u64>().unwrap();
                self.found_breakpoint(file, line);
            }

            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                let file = cap[1].to_string();
                let line = cap[2].parse::<u64>().unwrap();
                self.notifier
                    .lock()
                    .unwrap()
                    .jump_to_position(file.clone(), line);
            }

            for _ in RE_PROCESS_EXITED.captures_iter(line) {
                self.notifier
                    .lock()
                    .unwrap()
                    .signal_exited(self.pid.unwrap().as_raw() as u64, 0);
            }

            for cap in RE_PROCESS_EXITED_WITH_CODE.captures_iter(line) {
                let exit_code = cap[1].parse::<i64>().unwrap();
                self.notifier
                    .lock()
                    .unwrap()
                    .signal_exited(self.pid.unwrap().as_raw() as u64, exit_code);
            }
        }

        match self.status.clone() {
            PDBStatus::ReadyToPrint(var) => {
                self.status = PDBStatus::Printing(var);
            }
            PDBStatus::Printing(var) => {
                let listener_tx = self.listener_senders.remove(&PDBInput::PrintVariable(var)).unwrap();
                let to = data.len() - 2;
                tokio::spawn(
                    listener_tx
                        .send(PDBOutput::PrintVariable(data[0..to].to_string()))
                        .map(|_| {})
                        .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
                );
            }
            _ => {}
        }
    }

    fn found_breakpoint(&mut self, file: String, line: u64) {
        let pdb_input = PDBInput::Breakpoint(file.clone(), line);

        self.notifier
            .lock()
            .unwrap()
            .breakpoint_set(file.clone(), line);

        match self.listener_senders.remove(&pdb_input) {
            Some(listener_tx) => {
                tokio::spawn(
                    listener_tx
                        .send(PDBOutput::Breakpoint(file, line))
                        .map(|_| {})
                        .map_err(|e| eprintln!("Error sending to analyser: {}", e)),
                );
            }
            None => {}
        }
    }

    fn send(&self, data: &str) {
        let tx = self.in_tx.clone().unwrap();
        let data = data.to_string() + "\n";

        tokio::spawn(tx.send(Bytes::from(data)).map(|_| {}).map_err(|e| {
            eprintln!("Error writing data to pdb: {:?}", e);
        }));
    }

    fn send_and_receive(
        &mut self,
        msg: PDBInput,
    ) -> Box<dyn Future<Item = PDBOutput, Error = io::Error> + Send> {
        let (listener_tx, listener_rx) = mpsc::channel(1);
        self.listener_senders.insert(msg.clone(), listener_tx);

        match self.in_tx {
            None => {
                return Box::new(future::lazy(move || Ok(PDBOutput::None)));
            }
            _ => {}
        }

        match msg {
            PDBInput::Breakpoint(file, line_num) => {
                self.send(&format!("break {}:{}", file, line_num));
            },
            PDBInput::PrintVariable(variable) => {
                self.send(&format!("print({})", variable));
            }
        }

        let f = listener_rx
            .take(1)
            .into_future()
            .map(move |response| {
                println!("Response: {:?}", response);
                response.0.unwrap()
            })
            .map_err(|e| {
                eprintln!("Error reading pdb: {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Error reading pdb")
            });

        Box::new(f)
    }
}
