use std::sync::{Arc, Mutex};

use crate::notifier::Notifier;
use crate::request::RequestError;

#[derive(Debug)]
pub enum DebuggerInstruction {
    Run,
    Breakpoint,
}

#[derive(Debug)]
enum DebuggerState {
    Stopped,
    Paused(String, u32),
    Running,
    Error,
}

#[derive(Debug)]
pub struct PadreDebugger {
    state: DebuggerState,
    notifier: Arc<Mutex<Notifier>>,
}

impl PadreDebugger {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> PadreDebugger {
        PadreDebugger {
            state: DebuggerState::Stopped,
            notifier,
        }
    }

    pub fn ping(&self) -> Result<serde_json::Value, RequestError> {
        let pong = serde_json::json!({"ping":"pong"});
        Ok(pong)
    }

    pub fn pongs(&self) -> Result<serde_json::Value, RequestError> {
        let pong = serde_json::json!({"pong":"pongs"});
        Ok(pong)
    }
}
