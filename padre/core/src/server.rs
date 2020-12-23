/// Shared data structures mostly, largely handled in cli
use std::fmt;
use std::io;

/// Log level to log at, clients can choose to filter messages at certain log
/// levels
#[derive(Debug)]
pub enum LogLevel {
    CRITICAL = 1,
    ERROR,
    WARN,
    INFO,
    DEBUG,
}

/// A notification to be sent to all listeners of an event
///
/// Takes a String as the command and a vector of JSON values as arguments. For example, a
/// `Notication` with a command `execute` and vector arguments TODO...
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    cmd: String,
    args: Vec<serde_json::Value>,
}

impl Notification {
    /// Create a notification
    pub fn new(cmd: String, args: Vec<serde_json::Value>) -> Self {
        Notification { cmd, args }
    }

    /// Return the notification cmd
    pub fn cmd(&self) -> &str {
        self.cmd.as_ref()
    }

    /// Return the response values
    pub fn args(&self) -> &Vec<serde_json::Value> {
        &self.args
    }
}

#[derive(Debug)]
pub enum PadreErrorKind {
    GenericError,
    RequestSyntaxError,
    ProcessSpawnError,
    DebuggerError,
}

#[derive(Debug)]
pub struct PadreError {
    kind: PadreErrorKind,
    error_string: String,
    debug_string: String,
}

impl PadreError {
    pub fn new(kind: PadreErrorKind, error_string: String, debug_string: String) -> Self {
        PadreError {
            kind,
            error_string,
            debug_string,
        }
    }

    pub fn get_error_string(&self) -> &str {
        &self.error_string
    }

    pub fn get_debug_string(&self) -> &str {
        &self.debug_string
    }
}

impl fmt::Display for PadreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid first item to double")
    }
}

impl From<io::Error> for PadreError {
    fn from(err: io::Error) -> PadreError {
        PadreError::new(
            PadreErrorKind::GenericError,
            "Generic error".to_string(),
            format!("Generic error {}", err),
        )
    }
}
