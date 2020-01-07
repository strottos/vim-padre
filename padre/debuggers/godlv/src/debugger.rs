//! Go Delve debugger
//!
//! The main Delve Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use super::process::{Message, Process};
use padre_core::notifier::{log_msg, LogLevel};
use padre_core::server::{DebuggerV1, FileLocation, Variable};

use tokio::sync::oneshot;

#[derive(Debug)]
pub struct ImplDebugger {
    process: Arc<Mutex<Process>>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {
            process: Arc::new(Mutex::new(Process::new(debugger_cmd, run_cmd))),
        }
    }
}

impl DebuggerV1 for ImplDebugger {
    fn setup(&mut self) {
        // Awakener
        let (tx, rx) = oneshot::channel();

        self.process.lock().unwrap().add_awakener(tx);

        let process = self.process.clone();

        tokio::spawn(async move {
            rx.await.unwrap();
            let (tx, rx) = oneshot::channel();
            process.lock().unwrap().add_awakener(tx);

            let process2 = process.clone();

            tokio::spawn(async move {
                rx.await.unwrap();
                process2.lock().unwrap().send_msg(Message::Continue);
            });

            process.lock().unwrap().send_msg(Message::Custom("b main.main".to_string()));
        });

        let process = self.process.clone();

        // Sleep just to make sure vim has had time to connect
        sleep(Duration::new(1, 0));

        tokio::spawn(async move {
            process.lock().unwrap().run();
        });
    }

    fn teardown(&mut self) {
        exit(0);
    }

    /// Run Delve and perform any setup necessary
    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");
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

        self.process.lock().unwrap().send_msg(Message::Breakpoint(file_location));
    }

    fn unbreakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::StepIn);
    }

    fn step_over(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::StepOver);
    }

    fn continue_(&mut self, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::Continue);
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        self.process.lock().unwrap().send_msg(Message::PrintVariable(variable.clone()));
    }
}
