//! Debugger Module
//!
//! Main module for handling the debuggers, defines the standard versioned debugger interfaces
//! and creates the main debugger objects.

use std::fmt::Debug;
use std::io;

use crate::util::{file_is_binary_executable, file_is_text, get_file_full_path};

use tokio::prelude::*;

mod lldb;

/// Debugger trait that implements the basics
pub trait Debugger: Debug {
    fn setup(&mut self);
    fn teardown(&mut self);
    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn breakpoint(
        &mut self,
        file: &str,
        line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn continue_(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn print(
        &mut self,
        variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
}

/// Get the debugger implementation
///
/// If the debugger type is not specified it will try it's best to guess what kind of debugger to
/// return.
pub fn get_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
) -> Box<dyn Debugger + Send> {
    let debugger_type = match debugger_type {
        Some(s) => s.to_string(),
        None => match debugger_cmd {
            Some(s) => get_debugger_type(s).expect("Can't find debugger type, bailing"),
            None => panic!("Couldn't find debugger, try specifying with -t or -d"),
        },
    };

    let debugger_cmd = match debugger_cmd {
        Some(s) => s.to_string(),
        None => debugger_type.clone(),
    };

    let mut debugger: Box<dyn Debugger + Send> = match debugger_type.to_ascii_lowercase().as_ref() {
        "lldb" => Box::new(lldb::ImplDebugger::new(debugger_cmd, run_cmd)),
        //        "node" => Box::new(node::ImplDebugger::new(
        //            debugger_cmd,
        //            run_cmd,
        //        )),
        //        "python" => Box::new(python::ImplDebugger::new(
        //            debugger_cmd,
        //            run_cmd,
        //        )),
        _ => panic!("Can't build debugger type {}, panicking", &debugger_type),
    };

    debugger.setup();

    debugger
}

/// Guesses the debugger type
pub fn get_debugger_type(cmd: &str) -> Option<String> {
    let cmd = get_file_full_path(cmd);
    if is_node(&cmd) {
        Some(String::from("node"))
    } else if is_lldb(&cmd) {
        Some(String::from("lldb"))
    } else {
        None
    }
}

/// Checks if the file is a binary executable
fn is_lldb(cmd: &str) -> bool {
    if file_is_binary_executable(cmd) {
        return true;
    }

    false
}

/// Checks if the file is a NodeJS script
fn is_node(cmd: &str) -> bool {
    if file_is_text(cmd) && cmd.ends_with(".js") {
        return true;
    }

    if file_is_binary_executable(cmd) && cmd.contains("node") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    fn get_test_path_env_var() -> String {
        format!(
            "{}:{}:/bin:/usr/bin",
            Path::new("./test_files")
                .canonicalize()
                .expect("Cannot find test_files directory")
                .as_path()
                .to_str()
                .unwrap(),
            Path::new("./integration/test_files")
                .canonicalize()
                .expect("Cannot find test_files directory")
                .as_path()
                .to_str()
                .unwrap(),
        )
    }

    #[test]
    fn finds_lldb_when_specified_and_absolute_path() {
        assert_eq!(
            super::get_debugger_type("./test_files/lldb-server"),
            Some(String::from("lldb"))
        );
    }
}
