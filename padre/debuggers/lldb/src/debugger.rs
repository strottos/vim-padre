//! lldb client debugger
//!
//! The main LLDB Debugger entry point. Handles listening for instructions and
//! communicating through the `LLDBProcess`.

use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::process::{LLDBProcess, Message};
use padre_core::debugger::{Debugger, DebuggerCmd, DebuggerCmdBasic, FileLocation, Variable};
use padre_core::notifier::{log_msg, LogLevel};

use futures::prelude::*;
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
    /// Perform any initial setup including starting LLDB and setting up the stdio analyser stuff
    /// - startup lldb and setup the stdio analyser
    /// - perform initial setup so we can analyse LLDB properly
    #[allow(unreachable_patterns)]
    fn setup_handler(&self, mut queue_rx: mpsc::Receiver<(DebuggerCmd, Instant)>) {
        let debugger_cmd = self.debugger_cmd.clone();
        let run_cmd = self.run_cmd.clone();

        tokio::spawn(async move {
            let mut debugger = LLDBDebugger::new(debugger_cmd, run_cmd);

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
pub struct LLDBDebugger {
    process: Arc<Mutex<LLDBProcess>>,
}

impl LLDBDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        let (tx, rx) = oneshot::channel();

        let process = Arc::new(Mutex::new(LLDBProcess::new(
            debugger_cmd,
            run_cmd,
            Some(tx),
        )));

        let debugger = LLDBDebugger {
            process: process.clone(),
        };

        // Send a lot of startup messages to LLDB when ready
        tokio::spawn(async move {
            rx.await.unwrap();
            process
                .clone()
                .lock()
                .unwrap()
                .send_msg(Message::LLDBSetup, None);
        });

        debugger
    }

    fn interrupt(&mut self) {}

    fn teardown(&mut self) {
        self.process.lock().unwrap().stop();
        exit(0);
    }

    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::ProcessLaunching, None);
    }

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        log_msg(
            LogLevel::INFO,
            &format!(
                "Setting breakpoint in file {} at line number {}",
                file_location.name(),
                file_location.line_num()
            ),
        );

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::Breakpoint(file_location.clone()), None);
    }

    fn unbreakpoint(&mut self, _file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::StepIn, None);
    }

    fn step_over(&mut self, _timeout: Instant) {
        self.process
            .lock()
            .unwrap()
            .send_msg(Message::StepOver, None);
    }

    fn continue_(&mut self, _timeout: Instant) {
        self.process
            .lock()
            .unwrap()
            .send_msg(Message::Continue, None);
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        self.process
            .lock()
            .unwrap()
            .send_msg(Message::PrintVariable(variable.clone()), None);
    }
}
