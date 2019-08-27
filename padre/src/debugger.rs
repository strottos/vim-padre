//! Debugger Module
//!
//! Main module for handling the debuggers, defines the standard versioned debugger interfaces
//! and creates the main debugger objects.

use std::fmt::Debug;
use std::io;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::util::{file_is_binary_executable, file_is_text};

use tokio::prelude::*;

mod lldb;
mod node;
mod python;

/// Debuggers
#[derive(Debug)]
enum DebuggerType {
    LLDB,
    Node,
    Python,
}

/// File location
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct FileLocation {
    name: String,
    line_num: u64,
}

impl FileLocation {
    pub fn new(name: String, line_num: u64) -> Self {
        FileLocation { name, line_num }
    }
}

/// Variable name
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new(name: String) -> Self {
        Variable { name }
    }
}

/// All debugger commands
#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum DebuggerCmd {
    V1(DebuggerCmdV1),
}

/// All V1 debugger commands
#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum DebuggerCmdV1 {
    Run,
    Breakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    Print(Variable),
}

#[derive(Debug)]
pub struct Debugger {
    debugger: Box<dyn DebuggerV1 + Send>,
}

impl Debugger {
    pub fn new(debugger: Box<dyn DebuggerV1 + Send>) -> Debugger {
        Debugger { debugger }
    }

    pub fn stop(&mut self) {
        self.debugger.teardown();
    }

    pub fn handle_v1_cmd(
        &mut self,
        cmd: &DebuggerCmdV1,
        config: Arc<Mutex<Config>>,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        match cmd {
            DebuggerCmdV1::Run => self.debugger.run(config),
            DebuggerCmdV1::Breakpoint(fl) => self.debugger.breakpoint(fl, config),
            DebuggerCmdV1::StepIn => self.debugger.step_in(),
            DebuggerCmdV1::StepOver => self.debugger.step_over(),
            DebuggerCmdV1::Continue => self.debugger.continue_(),
            DebuggerCmdV1::Print(v) => self.debugger.print(v, config),
        }
    }
}

/// Debugger trait that implements the basics
pub trait DebuggerV1: Debug {
    fn setup(&mut self);
    fn teardown(&mut self);
    fn run(
        &mut self,
        config: Arc<Mutex<Config>>,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn breakpoint(
        &mut self,
        file_location: &FileLocation,
        config: Arc<Mutex<Config>>,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn continue_(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn print(
        &mut self,
        variable: &Variable,
        config: Arc<Mutex<Config>>,
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
) -> Debugger {
    let debugger_type = match debugger_type {
        Some(s) => match s.to_ascii_lowercase().as_str() {
            "lldb" => DebuggerType::LLDB,
            "python" => DebuggerType::Python,
            "node" => DebuggerType::Node,
            _ => panic!("Couldn't understand debugger type {}", s),
        },
        None => match get_debugger_type(&run_cmd[0]) {
            Some(s) => s,
            None => match debugger_cmd {
                Some(s) => match s {
                    "lldb" => DebuggerType::LLDB,
                    "python" | "python3" => DebuggerType::Python,
                    "node" => DebuggerType::Node,
                    _ => panic!(
                        "Can't find debugger type for {}, try specifying with -d or -t",
                        s
                    ),
                },
                None => panic!("Can't find debugger type, try specifying with -d or -t"),
            },
        },
    };

    let debugger_cmd = match debugger_cmd {
        Some(s) => s.to_string(),
        None => match debugger_type {
            DebuggerType::LLDB => "lldb".to_string(),
            DebuggerType::Node => "node".to_string(),
            DebuggerType::Python => "python3".to_string(),
        },
    };

    let mut debugger: Box<dyn DebuggerV1 + Send> = match debugger_type {
        DebuggerType::LLDB => Box::new(lldb::ImplDebugger::new(debugger_cmd, run_cmd)),
        DebuggerType::Node => Box::new(node::ImplDebugger::new(debugger_cmd, run_cmd)),
        DebuggerType::Python => Box::new(python::ImplDebugger::new(debugger_cmd, run_cmd)),
    };

    debugger.setup();

    Debugger::new(debugger)
}

/// Guesses the debugger type
fn get_debugger_type(run_cmd: &str) -> Option<DebuggerType> {
    if is_node(&run_cmd) {
        Some(DebuggerType::Node)
    } else if is_python(&run_cmd) {
        Some(DebuggerType::Python)
    } else if is_lldb(&run_cmd) {
        Some(DebuggerType::LLDB)
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

    // if file_is_binary_executable(cmd) && cmd.contains("node") {
    //     return true;
    // }

    false
}

/// Checks if the file is a NodeJS script
fn is_python(cmd: &str) -> bool {
    if file_is_text(cmd) && cmd.ends_with(".py") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("python") {
    //     return true;
    // }

    false
}
