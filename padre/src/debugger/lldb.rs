//! lldb client debugger

use std::sync::{Arc, Mutex};

use crate::request::{RequestError, Response};
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

mod lldb_process;

pub struct LLDB {
    notifier: Arc<Mutex<Notifier>>,
    started: bool,
    process: lldb_process::LLDBProcess,
}

impl Debugger for LLDB {
    fn start(&mut self, debugger_command: String, run_command: &Vec<String>) {
        self.process.start_process(debugger_command, run_command);
        self.started = true;
    }

    fn has_started(&self) -> bool {
        self.started
    }

    fn stop(&self) {
        println!("STOP");
    }

    fn breakpoint(&self, file: String, line_num: u32) -> Result<Response<Option<String>>, RequestError> {
        Ok(Response::PENDING(None))
    }
}

impl LLDB {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> LLDB {
        let process_notifier_clone = notifier.clone();
        LLDB {
            notifier: notifier,
            started: false,
            process: lldb_process::LLDBProcess::new(process_notifier_clone),
        }
    }
}
