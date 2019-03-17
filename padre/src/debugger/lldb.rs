//! lldb client debugger

use std::error::Error;
use std::fmt;
use std::io;
use std::io::{BufRead};
use std::sync::{Arc, Condvar, mpsc, Mutex};
use std::sync::mpsc::SyncSender;
use std::thread;

use crate::request::{RequestError, Response};
use crate::debugger::Debugger;
use crate::notifier::Notifier;

mod lldb_process;

pub enum LLDBStop {
    BREAKPOINT,
    STEP_IN,
    STEP_OVER,
}

#[derive(Debug)]
pub struct LLDBError {
    msg: String,
    debug: String,
}

impl fmt::Display for LLDBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}", self.msg)
    }
}

impl Error for LLDBError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl LLDBError {
    fn new(msg: String, debug: String) -> LLDBError {
        LLDBError {
            msg: msg,
            debug: debug,
        }
    }

    fn get_debug_info(&self) -> &str {
        &self.debug
    }
}

pub struct LLDB {
    notifier: Arc<Mutex<Notifier>>,
    started: bool,
    process: lldb_process::LLDBProcess,
    stop_listener: Arc<(Mutex<Option<Result<LLDBStop, LLDBError>>>, Condvar)>,
    sender: Option<SyncSender<String>>,
}

impl Debugger for LLDB {
    fn start(&mut self, debugger_command: String, run_command: &Vec<String>) {
        let (tx, rx) = mpsc::sync_channel(512);

        // Kick off lldb
        self.process.start_process(debugger_command, run_command, rx);

        // Send stdin to process
        thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap() + "\n";
                tx.send(line).unwrap();
            }
        });

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
        let stop_listener = Arc::new((Mutex::new(None), Condvar::new()));
        let stop_listener_process = stop_listener.clone();
        LLDB {
            notifier: notifier,
            started: false,
            process: lldb_process::LLDBProcess::new(
                process_notifier_clone,
                stop_listener_process,
            ),
            stop_listener: stop_listener,
            sender: None,
        }
    }
}
