//! lldb client debugger
//!
//! The main LLDB Debugger entry point. Handles listening for instructions and
//! communicating through the `LLDBProcess`.

use std::io;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::process::{LLDBProcess, Message};
use padre_core::config::Config;
use padre_core::notifier::{log_msg, LogLevel};
use padre_core::server::{DebuggerV1, FileLocation, Variable};

use bytes::Bytes;
use futures::prelude::*;
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

    fn step(&mut self, kind: &str) {
        //match self.check_process() {
        //    Some(f) => return f,
        //    _ => {}
        //}

        //let stmt = format!("thread {}\n", kind);

        //self.process.lock().unwrap().write_stdin(Bytes::from(stmt));

        //let f = future::lazy(move || {
        //    let resp = serde_json::json!({"status":"OK"});
        //    Ok(resp)
        //});
    }

    //fn check_process(
    //    &mut self,
    //) -> Option<Result<serde_json::Value, io::Error>> {
    //    match self.process.lock().unwrap().is_process_running() {
    //        false => {
    //            log_msg(LogLevel::WARN, "No process running");
    //            let f = future::lazy(move || {
    //                let resp = serde_json::json!({"status":"ERROR"});
    //                Ok(resp)
    //            });

    //            Some(Box::new(f))
    //        }
    //        true => None,
    //    }
    //}
}

impl DebuggerV1 for ImplDebugger {
    /// Perform any initial setup including starting LLDB and setting up the stdio analyser stuff
    /// - startup lldb and setup the stdio analyser
    /// - perform initial setup so we can analyse LLDB properly
    fn setup(&mut self) {
        let process = self.process.clone();

        tokio::spawn(async move {
            let msgs = [
                "settings set stop-line-count-after 0\n",
                "settings set stop-line-count-before 0\n",
                "settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n",
                "breakpoint set --name main\n",
            ];

            for msg in msgs.iter() {
                // Check we're actually listening
                let (tx, mut rx) = mpsc::channel(1);
                process.lock().unwrap().add_awakener(tx);
                rx.next().await.unwrap();
                process.lock().unwrap().drop_awakener();

                process.lock().unwrap().write_stdin(Bytes::from(msg.as_bytes()));
            }
        });

        self.process.lock().unwrap().setup();
    }

    fn teardown(&mut self) {
        self.process.lock().unwrap().teardown();
        exit(0);
    }

    fn run(&mut self, _timeout: Instant) {
        log_msg(LogLevel::INFO, "Launching process");

        //let (tx, rx) = mpsc::channel(1);

        //self.process
        //    .lock()
        //    .unwrap()
        //    .add_listener(Listener::Breakpoint, tx);

        //let process = self.process.clone();

        //let f = rx
        //    .take(1)
        //    .into_future()
        //    .and_then(move |lldb_output| {
        //        let lldb_output = lldb_output.0.unwrap();

        //        match lldb_output {
        //            Event::BreakpointSet(_) | Event::BreakpointMultiple => {}
        //            _ => {
        //                panic!("Don't understand output {:?}", lldb_output);
        //            }
        //        };

        //        Ok(())
        //    })
        //    .and_then(move |_| {
        //        let (tx, rx) = mpsc::channel(1);

        //        process
        //            .lock()
        //            .unwrap()
        //            .add_listener(Listener::ProcessLaunched, tx);

        //        process
        //            .lock()
        //            .unwrap()
        //            .write_stdin(Bytes::from("process launch\n"));

        //        rx.take(1).into_future()
        //    })
        //    .timeout(Duration::new(
        //        config
        //            .lock()
        //            .unwrap()
        //            .get_config("ProcessSpawnTimeout")
        //            .unwrap() as u64,
        //        0,
        //    ))
        //    .map(move |event| match event.0.unwrap() {
        //        Event::ProcessLaunched(pid) => {
        //            serde_json::json!({"status":"OK","pid":pid.to_string()})
        //        }
        //        _ => unreachable!(),
        //    })
        //    .map_err(|e| {
        //        eprintln!("Reading stdin error {:?}", e);
        //        io::Error::new(io::ErrorKind::Other, "Timed out spawning process")
        //    });

        //let stmt = "breakpoint set --name main\n";

        //self.process.lock().unwrap().write_stdin(Bytes::from(stmt));
    }

    fn breakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {
        log_msg(
            LogLevel::INFO,
            &format!(
                "Setting breakpoint in file {} at line number {}",
                file_location.name(),
                file_location.line_num()
            ),
        );

        self.process
            .lock()
            .unwrap()
            .send_msg(Message::Breakpoint(file_location.clone()));
    }

    fn unbreakpoint(&mut self, file_location: &FileLocation, _timeout: Instant) {}

    fn step_in(&mut self, _timeout: Instant) {
        self.step("step-in");
    }

    fn step_over(&mut self, _timeout: Instant) {
        self.step("step-over");
    }

    fn continue_(&mut self, _timeout: Instant) {
        self.step("continue");
    }

    fn print(&mut self, variable: &Variable, _timeout: Instant) {
        //match self.check_process() {
        //    Some(f) => return f,
        //    _ => {}
        //}

        //let (tx, rx) = mpsc::channel(1);

        //self.process
        //    .lock()
        //    .unwrap()
        //    .add_listener(Listener::PrintVariable, tx);

        //let f = rx
        //    .take(1)
        //    .into_future()
        //    .timeout(Duration::new(
        //        config
        //            .lock()
        //            .unwrap()
        //            .get_config("PrintVariableTimeout")
        //            .unwrap() as u64,
        //        0,
        //    ))
        //    .map(move |event| match event.0.unwrap() {
        //        Event::PrintVariable(variable, value) => serde_json::json!({
        //            "status": "OK",
        //            "variable": variable.name,
        //            "value": value.value(),
        //            "type": value.type_()
        //        }),
        //        Event::VariableNotFound(variable) => {
        //            log_msg(
        //                LogLevel::WARN,
        //                &format!("variable '{}' doesn't exist here", variable.name),
        //            );
        //            serde_json::json!({"status":"ERROR"})
        //        }
        //        _ => unreachable!(),
        //    })
        //    .map_err(|e| {
        //        eprintln!("Reading stdin error {:?}", e);
        //        io::Error::new(io::ErrorKind::Other, "Timed out printing variable")
        //    });

        //let stmt = format!("frame variable {}\n", variable.name);

        //self.process.lock().unwrap().write_stdin(Bytes::from(stmt));
    }
}
