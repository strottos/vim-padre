//! lldb client debugger
//!
//! The main LLDB Debugger entry point. Handles listening for instructions and
//! communicating through the `LLDBProcess`.

use std::io;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::process::{Event, LLDBProcess, Listener};
use crate::config::Config;
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
    /// Perform any initial setup including starting LLDB and setting up the stdio analyser stuff
    /// - startup lldb and setup the stdio analyser
    /// - perform initial setup so we can analyse LLDB properly
    fn setup(&mut self) {
        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(Listener::LLDBLaunched, tx);

        let process = self.process.clone();

        tokio::spawn(
            rx.take(1)
                .for_each(move |event| {
                    match event {
                        Event::LLDBLaunched => {
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

    fn run(
        &mut self,
        config: Arc<Mutex<Config>>,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        log_msg(LogLevel::INFO, "Launching process");

        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(Listener::Breakpoint, tx);

        let process = self.process.clone();

        let f = rx
            .take(1)
            .into_future()
            .and_then(move |lldb_output| {
                let lldb_output = lldb_output.0.unwrap();

                match lldb_output {
                    Event::BreakpointSet(_) | Event::BreakpointMultiple => {}
                    _ => {
                        panic!("Don't understand output {:?}", lldb_output);
                    }
                };

                Ok(())
            })
            .and_then(move |_| {
                let (tx, rx) = mpsc::channel(1);

                process
                    .lock()
                    .unwrap()
                    .add_listener(Listener::ProcessLaunched, tx);

                process
                    .lock()
                    .unwrap()
                    .write_stdin(Bytes::from("process launch\n"));

                rx.take(1).into_future()
            })
            .timeout(Duration::new(
                config
                    .lock()
                    .unwrap()
                    .get_config("ProcessSpawnTimeout")
                    .unwrap() as u64,
                0,
            ))
            .map(move |event| match event.0.unwrap() {
                Event::ProcessLaunched(pid) => {
                    serde_json::json!({"status":"OK","pid":pid.to_string()})
                }
                _ => unreachable!(),
            })
            .map_err(|e| {
                eprintln!("Reading stdin error {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out spawning process")
            });

        let stmt = "breakpoint set --name main\n";

        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        file_location: &FileLocation,
        config: Arc<Mutex<Config>>,
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
            .add_listener(Listener::Breakpoint, tx);

        let f = rx
            .take(1)
            .into_future()
            .timeout(Duration::new(
                config
                    .lock()
                    .unwrap()
                    .get_config("BreakpointTimeout")
                    .unwrap() as u64,
                0,
            ))
            .map(move |event| match event.0.unwrap() {
                Event::BreakpointSet(_) => serde_json::json!({"status":"OK"}),
                Event::BreakpointPending => serde_json::json!({"status":"PENDING"}),
                _ => unreachable!(),
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
        self.step("step-in")
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.step("step-over")
    }

    fn continue_(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.step("continue")
    }

    fn print(
        &mut self,
        variable: &Variable,
        config: Arc<Mutex<Config>>,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        match self.check_process() {
            Some(f) => return f,
            _ => {}
        }

        let (tx, rx) = mpsc::channel(1);

        self.process
            .lock()
            .unwrap()
            .add_listener(Listener::PrintVariable, tx);

        let f = rx
            .take(1)
            .into_future()
            .timeout(Duration::new(
                config
                    .lock()
                    .unwrap()
                    .get_config("PrintVariableTimeout")
                    .unwrap() as u64,
                0,
            ))
            .map(move |event| match event.0.unwrap() {
                Event::PrintVariable(variable, value) => serde_json::json!({
                    "status": "OK",
                    "variable": variable.name,
                    "value": value.value(),
                    "type": value.type_()
                }),
                Event::VariableNotFound(variable) => {
                    log_msg(
                        LogLevel::WARN,
                        &format!("variable '{}' doesn't exist here", variable.name),
                    );
                    serde_json::json!({"status":"ERROR"})
                }
                _ => unreachable!(),
            })
            .map_err(|e| {
                eprintln!("Reading stdin error {:?}", e);
                io::Error::new(io::ErrorKind::Other, "Timed out printing variable")
            });

        let stmt = format!("frame variable {}\n", variable.name);

        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

        Box::new(f)
    }
}

impl ImplDebugger {
    fn step(
        &mut self,
        kind: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        match self.check_process() {
            Some(f) => return f,
            _ => {}
        }

        let stmt = format!("thread {}\n", kind);

        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn check_process(
        &mut self,
    ) -> Option<Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>> {
        match self.process.lock().unwrap().is_process_running() {
            false => {
                log_msg(LogLevel::WARN, "No process running");
                let f = future::lazy(move || {
                    let resp = serde_json::json!({"status":"ERROR"});
                    Ok(resp)
                });

                Some(Box::new(f))
            }
            true => None,
        }
    }
}
