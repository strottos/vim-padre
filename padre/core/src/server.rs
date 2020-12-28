//! Server
//!
//! Handles parsing request messages and forwarding to padre and debuggers for actioning.

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


enum PadreRequest {
    PadreCmd {
        Ping,
        Pings,
        GetConfig(String),
        SetConfig(String, String),
    }
    DebuggerCmd {
        Run,
        Breakpoint(FileLocation),
        StepIn,
        StepOver,
        Continue,
        Print(Variable),
    }
}
