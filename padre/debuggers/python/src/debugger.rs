//! Python debugger
//!
//! The main Python Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::process::{Message, PDBStatus, Process};
use padre_core::notifier::{log_msg, LogLevel};
use padre_core::server::{FileLocation, Variable};

use futures::StreamExt;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImplDebugger {
    process: Arc<Mutex<Process>>,
    pending_breakpoints: Option<Vec<FileLocation>>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {
            process: Arc::new(Mutex::new(Process::new(debugger_cmd, run_cmd))),
            pending_breakpoints: Some(vec![]),
        }
    }

    pub fn teardown(&mut self) {
        exit(0);
    }

    /// Run python and perform any setup necessary
    pub fn run(&mut self, timeout: Instant) {
        match self.process.lock().unwrap().get_status() {
            PDBStatus::None => {}
            _ => {
                let msg = "Process already running, not launching";
                eprintln!("{}", msg);
                log_msg(LogLevel::WARN, msg);
                return;
            }
        }

        log_msg(LogLevel::INFO, "Launching process");

        let pending_breakpoints = self.pending_breakpoints.take();

        let process = self.process.clone();

        tokio::spawn(async move {
            match pending_breakpoints {
                Some(pbs) => {
                    for pb in pbs {
                        // Check we're actually listening
                        let (tx, mut rx) = mpsc::channel(1);
                        process.lock().unwrap().add_awakener(tx);
                        rx.next().await.unwrap();
                        process.lock().unwrap().drop_awakener();

                        // And send the breakpoint info
                        process.lock().unwrap().send_msg(Message::Breakpoint(pb));
                    }
                }
                None => {}
            };
        });

        let process = self.process.clone();

        tokio::spawn(async move {
            process.lock().unwrap().run();
        });
    }

    pub fn breakpoint(&mut self, file_location: &FileLocation, timeout: Instant) {
        let full_file_path = PathBuf::from(format!("{}", file_location.name()));
        let full_file_name = full_file_path.canonicalize().unwrap();
        let file_location = FileLocation::new(
            full_file_name.to_str().unwrap().to_string(),
            file_location.line_num(),
        );

        log_msg(
            LogLevel::INFO,
            &format!(
                "Setting breakpoint in file {} at line number {}",
                file_location.name(),
                file_location.line_num()
            ),
        );

        // If not started yet add as a pending breakpoint that will get set during run period.
        match self.process.lock().unwrap().get_status() {
            PDBStatus::None => {
                match self.pending_breakpoints {
                    Some(ref mut x) => x.push(file_location.clone()),
                    None => {}
                };

                log_msg(
                    LogLevel::INFO,
                    &format!(
                        "Breakpoint pending in file {} at line number {}",
                        file_location.name(),
                        file_location.line_num()
                    ),
                );

                return;
            }
            _ => {}
        }

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::Breakpoint(file_location));
    }

    pub fn step_in(&mut self, timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::StepIn);
    }

    pub fn step_over(&mut self, timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::StepOver);
    }

    pub fn continue_(&mut self, timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::Continue);
    }

    pub fn print(&mut self, variable: &Variable, timeout: Instant) {
        //        //match self.check_process_running() {
        //        //    Some(f) => return f,
        //        //    None => {}
        //        //};
        //
        //        let (tx, rx) = mpsc::channel(1);
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .set_status(PDBStatus::Printing(variable.clone()));
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .add_listener(Listener::PrintVariable, tx);
        //
        //        let f = rx
        //            .take(1)
        //            .into_future()
        //            .timeout(Duration::new(
        //                config
        //                    .lock()
        //                    .unwrap()
        //                    .get_config("PrintVariableTimeout")
        //                    .unwrap() as u64,
        //                0,
        //            ))
        //            .map(move |event| match event.0.unwrap() {
        //                Event::PrintVariable(variable, value) => serde_json::json!({
        //                    "status": "OK",
        //                    "variable": variable.name,
        //                    "value": value,
        //                }),
        //                _ => unreachable!(),
        //            })
        //            .map_err(|e| {
        //                eprintln!("Reading stdin error {:?}", e);
        //                io::Error::new(io::ErrorKind::Other, "Timed out printing variable")
        //            });

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::PrintVariable(variable.clone()));
    }
}