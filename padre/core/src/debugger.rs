//! Debugger Module
//!
//! Main module for handling the debuggers, defines the standard versioned debugger interfaces.

use std::fmt::Debug;
use std::time::Instant;

use tokio::sync::mpsc::Receiver;

// TODO: Get some of this out of pub use and just in this module??

/// Debugger trait that implements the basics
pub trait Debugger: Debug {
    fn setup_handler(&self, queue_rx: Receiver<(DebuggerCmd, Instant)>);
    fn teardown(&mut self);
}

/// All debugger commands
#[derive(Clone, Debug, PartialEq)]
pub enum DebuggerCmd {
    V1(DebuggerCmdV1),
}

/// All debugger commands
#[derive(Clone, Debug, PartialEq)]
pub enum DebuggerCmdV1 {
    Run,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    Print(Variable),
}

/// File location
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileLocation {
    name: String,
    line_num: u64,
}

impl FileLocation {
    pub fn new(name: String, line_num: u64) -> Self {
        FileLocation { name, line_num }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn line_num(&self) -> u64 {
        self.line_num
    }
}

/// Variable name
#[derive(Clone, Debug)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new(name: String) -> Self {
        Variable { name }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Variable {}
