//! The Python debugger module

#[macro_use]
extern crate lazy_static;

use std::time::Instant;

use tokio::sync::{mpsc, oneshot};

use padre_core::debugger::DebuggerCmd;
use padre_core::server::Notification;
use padre_core::Result;

mod debugger;
mod process;

pub fn get_debugger(
    debugger_cmd: String,
    run_cmd: Vec<String>,
    queue_rx: mpsc::Receiver<(
        DebuggerCmd,
        Instant,
        oneshot::Sender<Result<serde_json::Value>>,
    )>,
    notifier_tx: mpsc::Sender<Notification>,
) -> debugger::PythonDebugger {
    debugger::PythonDebugger::new(debugger_cmd, run_cmd, queue_rx, notifier_tx)
}
