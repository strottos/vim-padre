//! Node debugger

use std::sync::{Arc, Mutex};

use crate::debugger::Debugger;
use crate::notifier::Notifier;
use crate::request::RequestError;

use tokio::prelude::*;

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {}

    fn run(&mut self) -> Result<serde_json::Value, RequestError> {
        let ret = serde_json::json!({"status":"OK"});
        Ok(ret)
    }

    fn breakpoint(
        &mut self,
        file: String,
        line_num: u64,
    ) -> Result<serde_json::Value, RequestError> {
        let ret = serde_json::json!({"status":"OK"});
        Ok(ret)
    }
}
