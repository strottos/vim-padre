//! Go Delve debugger
//!
//! The main Delve Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::process::{Message, DlvProcess};
use padre_core::debugger::{Debugger, DebuggerCmd, DebuggerCmdBasic, FileLocation, Variable};
use padre_core::notifier::{log_msg, LogLevel};

use futures::StreamExt;
use tokio::time::delay_for;
use tokio::sync::{mpsc, oneshot};

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
    fn setup_handler(&self, mut queue_rx: mpsc::Receiver<(DebuggerCmd, Instant)>) {
        let debugger_cmd = self.debugger_cmd.clone();
        let run_cmd = self.run_cmd.clone();

        tokio::spawn(async move {
            let mut debugger = DlvDebugger::new(debugger_cmd, run_cmd);

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
                        log_msg(LogLevel::WARN, &format!("Got a command that wasn't understood {:?}", cmd));
                    }
                };
            }

            exit(0);
        });
    }
}

struct DlvDebugger {
    process: Arc<Mutex<DlvProcess>>,
}

impl DlvDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        let (tx, rx) = oneshot::channel();

        let process = Arc::new(Mutex::new(DlvProcess::new(
            debugger_cmd,
            run_cmd,
            Some(tx),
        )));

        let debugger = DlvDebugger {
            process: process.clone(),
        };

        // Send a lot of startup messages to LLDB when ready
        tokio::spawn(async move {
            rx.await.unwrap();
            // Mini sleep to make sure VIM has connected
            thread::sleep(Duration::from_millis(500));
            process
                .clone()
                .lock()
                .unwrap()
                .send_msg(Message::DlvSetup, None);
        });

        debugger
    }

    fn setup(&mut self) {
        // Awakener
//        let (tx, rx) = oneshot::channel();
//
//        self.process.lock().unwrap().add_awakener(tx);
//
//        let process = self.process.clone();
//
//        tokio::spawn(async move {
//            rx.await.unwrap();
//            let (tx, rx) = oneshot::channel();
//            process.lock().unwrap().add_awakener(tx);
//
//            let process2 = process.clone();
//
//            tokio::spawn(async move {
//                rx.await.unwrap();
//                process2.lock().unwrap().send_msg(Message::Continue);
//            });
//
//            process.lock().unwrap().send_msg(Message::MainBreakpoint);
//        });
//
//        let process = self.process.clone();
//
//        tokio::spawn(async move {
//            // Sleep just to make sure vim has had time to connect
//            delay_for(Duration::new(1, 0)).await;
//
//            process.lock().unwrap().run();
//        });
    }

    /// Run Delve and perform any setup necessary
    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");

        self.process.lock().unwrap().send_msg(Message::ProcessLaunching, None);
    }

    fn interrupt(&mut self) {}

    fn teardown(&mut self) {
        self.process.lock().unwrap().stop();
    }

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        let full_file_path = PathBuf::from(format!("{}", file_location.name()));
        // TODO: Hard errors when doesn't exist
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

        self.process.lock().unwrap().send_msg(Message::Breakpoint(file_location), None);
    }

    fn unbreakpoint(&mut self, _file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::StepIn, None);
    }

    fn step_over(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::StepOver, None);
    }

    fn continue_(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::Continue, None);
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::PrintVariable(variable.clone()), None);
    }
}
