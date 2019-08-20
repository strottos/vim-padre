//! lldb client debugger

use std::io;
use std::process::exit;
use std::sync::{Arc, Mutex};

use super::process::{LLDBEvent, LLDBListener, LLDBProcess};
use crate::debugger::{DebuggerV1, FileLocation, Variable};
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
                .for_each(move |event| {
                    match event {
                        LLDBEvent::LLDBLaunched => {
                            process.lock().unwrap().write_stdin(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]));
                            process.lock().unwrap().write_stdin(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]));
                            process.lock().unwrap().write_stdin(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]));
                        }
                        _ => unreachable!()
                    }
                    Ok(())
                })
                .map_err(|e| {
                    eprintln!("Reading stdin error {:?}", e);
                })
        );

        self.process.lock().unwrap().setup();
    }

    fn teardown(&mut self) {
        self.process.lock().unwrap().teardown();
        exit(0);
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        log_msg(LogLevel::INFO, "Launching process");

        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(LLDBListener::Breakpoint, tx);

        let process = self.process.clone();

        let f = rx.take(1)
            .into_future()
            .and_then(move |lldb_output| {
                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    LLDBEvent::BreakpointSet(_) | LLDBEvent::BreakpointMultiple => {}
                    _ => {
                        panic!("Don't understand output {:?}", lldb_output);
                    }
                };

                Ok(())
            })
            .and_then(move |_| {
                let (tx, rx) = mpsc::channel(1);

                process.lock()
                    .unwrap()
                    .add_listener(LLDBListener::ProcessLaunched, tx);

                process.lock()
                    .unwrap()
                    .write_stdin(Bytes::from("process launch\n"));

                rx.take(1).into_future()
            })
            .map(move |event| {
                match event.0.unwrap() {
                    LLDBEvent::ProcessLaunched(pid) => {
                        println!("Process launched {:?}", pid);
                        serde_json::json!({"status":"OK","pid":pid.to_string()})
                    }
                    _ => unreachable!()
                }
            })
            .map_err(|e| {
                eprintln!("Reading stdin error {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out setting breakpoint")
            });

        let stmt = "breakpoint set --name main\n";

        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file_location: &FileLocation,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        log_msg(
            LogLevel::INFO,
            &format!(
                "Setting breakpoint in file {} at line number {}",
                file_location.file_name, file_location.line_num
            ),
        );

        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(LLDBListener::Breakpoint, tx);

        let f = rx.take(1)
            .into_future()
            .map(move |event| {
                match event.0.unwrap() {
                    LLDBEvent::BreakpointSet(fl) => {
                        println!("Breakpoint set {:?}", fl);
                        serde_json::json!({"status":"OK"})
                    }
                    LLDBEvent::BreakpointPending => {
                        println!("Breakpoint Pending");
                        serde_json::json!({"status":"PENDING"})
                    }
                    _ => unreachable!()
                }
            })
            .map_err(|e| {
                eprintln!("Reading stdin error {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out setting breakpoint")
            });

        let stmt = format!(
            "breakpoint set --file {} --line {}\n",
            file_location.file_name, file_location.line_num
        );

        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

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
        variable: &mut Variable,
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
