use crate::request::{RequestError, Response};

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
}

impl PadreDebugger {
    pub fn new() -> PadreDebugger {
        PadreDebugger {
            state: DebuggerState::Stopped,
        }
    }

    pub fn ping(&self) -> Result<serde_json::Value, RequestError> {
        let pong = serde_json::json!({"ping":"pong"});
        Ok(pong)
    }
}
