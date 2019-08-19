//! lldb client debugger

use std::io;
use std::sync::{Arc, Mutex};

use super::process::{LLDBEvent, LLDBListener, LLDBProcess};
use crate::debugger::DebuggerV1;
use crate::notifier::{log_msg, LogLevel};

use bytes::Bytes;
use tokio::prelude::*;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImplDebugger {
    process: Arc<Mutex<LLDBProcess>>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {
            process: Arc::new(Mutex::new(LLDBProcess::new(debugger_cmd, run_cmd))),
        }
    }
}

impl DebuggerV1 for ImplDebugger {
    /// Perform any initial setup including
    /// - startup lldb
    fn setup(&mut self) {
        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(LLDBListener::LLDBLaunched, tx);

        let process = self.process.clone();

        tokio::spawn(
            rx.take(1)
                .for_each(move |a| {
                    // TODO
                    process.lock().unwrap().write_stdin(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]));
                    process.lock().unwrap().write_stdin(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]));
                    process.lock().unwrap().write_stdin(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]));
                    Ok(())
                })
                .map_err(|e| {
                    eprintln!("Reading stdin error {:?}", e);
                })
        );

        self.process.lock().unwrap().setup();
    }

    fn teardown(&mut self) {}

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        log_msg(LogLevel::INFO, "Launching process".to_string());

        self.process
            .lock()
            .unwrap()
            .write_stdin(Bytes::from("process launch\n"));

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file: &str,
        line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn continue_(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn print(
        &mut self,
        variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }
}

#[cfg(test)]
mod tests {}
