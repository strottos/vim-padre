//! Go Delve debugger
//!
//! The main Delve Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

use std::path::PathBuf;
use std::process::exit;
use std::time::Instant;

use padre_core::notifier::{log_msg, LogLevel};
use padre_core::server::{DebuggerV1, FileLocation, Variable};

#[derive(Debug)]
pub struct ImplDebugger {}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {}
    }
}

impl DebuggerV1 for ImplDebugger {
    fn setup(&mut self) {}

    fn teardown(&mut self) {
        exit(0);
    }

    /// Run Delve and perform any setup necessary
    fn run(&mut self, _timeout: Instant) {}

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
    }

    fn unbreakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {}

    fn step_over(&mut self, _timeout: Instant) {}

    fn continue_(&mut self, _timeout: Instant) {}

    fn print(&mut self, variable: &Variable, _timeout: Instant) {}
}
