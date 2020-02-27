//! Python debugger
//!
//! The main Python Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::process::{Message, PDBStatus, Process};
use padre_core::debugger::{Debugger, DebuggerCmd, DebuggerCmdV1, FileLocation, Variable};
use padre_core::notifier::{log_msg, LogLevel};

use futures::StreamExt;
use tokio::sync::mpsc::{self, Receiver};

#[derive(Debug)]
pub struct ImplDebugger {
    debugger_cmd: String,
    run_cmd: Vec<String>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {
            debugger_cmd,
            run_cmd,
        }
    }
}

impl Debugger for ImplDebugger {
    #[allow(unreachable_patterns)]
    fn setup_handler(&self, mut queue_rx: Receiver<(DebuggerCmd, Instant)>) {
        let debugger_cmd = self.debugger_cmd.clone();
        let run_cmd = self.run_cmd.clone();

        tokio::spawn(async move {
            let mut debugger = PythonDebugger::new(debugger_cmd, run_cmd);

            while let Some(cmd) = queue_rx.next().await {
                match cmd.0 {
                    DebuggerCmd::V1(v1cmd) => match v1cmd {
                        DebuggerCmdV1::Run => debugger.run(cmd.1),
                        DebuggerCmdV1::Breakpoint(fl) => debugger.breakpoint(&fl, cmd.1),
                        DebuggerCmdV1::Unbreakpoint(fl) => debugger.unbreakpoint(&fl, cmd.1),
                        DebuggerCmdV1::StepIn => debugger.step_in(cmd.1),
                        DebuggerCmdV1::StepOver => debugger.step_over(cmd.1),
                        DebuggerCmdV1::Continue => debugger.continue_(cmd.1),
                        DebuggerCmdV1::Print(v) => debugger.print(&v, cmd.1),
                    },
                    _ => unimplemented!(),
                };
            }
        });
    }

    fn teardown(&mut self) {
        exit(0);
    }
}

struct PythonDebugger {
    process: Arc<Mutex<Process>>,
    pending_breakpoints: Option<Vec<FileLocation>>,
}

impl PythonDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> PythonDebugger {
        PythonDebugger {
            process: Arc::new(Mutex::new(Process::new(debugger_cmd, run_cmd))),
            pending_breakpoints: Some(vec![]),
        }
    }

    /// Run python and perform any setup necessary
    fn run(&mut self, _timeout: Instant) {
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

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        let full_file_path = PathBuf::from(format!("{}", file_location.name()));

        // TODO: What happens when it doesn't exist
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

    fn unbreakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        let full_file_path = PathBuf::from(format!("{}", file_location.name()));
        let full_file_name = full_file_path.canonicalize().unwrap();
        let file_location = FileLocation::new(
            full_file_name.to_str().unwrap().to_string(),
            file_location.line_num(),
        );

        log_msg(
            LogLevel::INFO,
            &format!(
                "Removing breakpoint in file {} at line number {}",
                file_location.name(),
                file_location.line_num()
            ),
        );

        // If not started yet remove any pending breakpoint that will get set during run period.
        match self.process.lock().unwrap().get_status() {
            PDBStatus::None => {
                match self.pending_breakpoints {
                    Some(ref mut x) => {
                        let mut index = None;
                        for (i, elem) in x.iter().enumerate() {
                            if *elem == file_location {
                                index = Some(i);
                            }
                        }
                        match index {
                            Some(i) => {
                                x.remove(i);
                            }
                            None => {}
                        };
                    }
                    None => {}
                };

                log_msg(
                    LogLevel::INFO,
                    &format!(
                        "Pending breakpoint removed in file {} at line number {}",
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
            .send_msg(Message::Unbreakpoint(file_location));
    }

    fn step_in(&mut self, _timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::StepIn);
    }

    fn step_over(&mut self, _timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::StepOver);
    }

    fn continue_(&mut self, _timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process.lock().unwrap().send_msg(Message::Continue);
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        //match self.check_process_running() {
        //    Some(f) => return f,
        //    None => {}
        //};

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::PrintVariable(variable.clone()));
    }
}
