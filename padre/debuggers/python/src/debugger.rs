//! Python debugger
//!
//! The main Python Debugger entry point. Handles listening for instructions and
//! communicating through the `Process`.

// use std::io;
// use std::path::PathBuf;
use std::process::exit;
use std::sync::{Arc, Mutex};
// use std::time::Duration;

use super::process::Process;
// use super::process::{Event, Listener, PDBStatus, Process};
// use padre_core::config::Config;
use padre_core::server::{FileLocation, Variable};
use padre_core::notifier::{log_msg, LogLevel};

//use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImplDebugger {
    process: Arc<Mutex<Process>>,
    pending_breakpoints: Option<Vec<FileLocation>>,
}

impl ImplDebugger {
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger {
            process: Arc::new(Mutex::new(Process::new(debugger_cmd, run_cmd))),
            pending_breakpoints: Some(vec![]),
        }
    }

    //fn check_process_running(
    //    &self,
    //) -> Option<Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>> {
    //    match self.process.lock().unwrap().get_status() {
    //        PDBStatus::None => {
    //            let f = future::lazy(move || {
    //                let resp = serde_json::json!({"status":"ERROR"});
    //                Ok(resp)
    //            });
    //            return Some(Box::new(f));
    //        }
    //        _ => None,
    //    }
    //}

    fn setup(&mut self) {}

    fn teardown(&mut self) {
        exit(0);
    }

    /// Run python and perform any setup necessary
    pub fn run<'a>(&mut self) {
        // // TODO: Find a better way of doing this, this madness
        // let pending_breakpoints = match self.pending_breakpoints.take() {
        //     Some(pb) => pb,
        //     None => {
        //         let msg = "Process already running, not launching";
        //         eprintln!("{}", msg);
        //         log_msg(LogLevel::WARN, msg);
        //         return Ok(serde_json::json!({"status":"ERROR"}));
        //     }
        // };

        log_msg(LogLevel::INFO, "Launching process");

        let process = self.process.clone();

        // TODO: Timeouts
        tokio::spawn(async move {
            process.lock().unwrap().run();
        });
    }

    pub fn breakpoint(
        &mut self,
        file_location: &FileLocation,
        //config: Arc<Mutex<Config>>,
    ) {
        //log_msg(
        //    LogLevel::INFO,
        //    &format!(
        //        "Setting breakpoint in file {} at line number {}",
        //        file_location.name, file_location.line_num
        //    ),
        //);

        //        // If not started yet add as a pending breakpoint that will get set during run period.
        //        match self.process.lock().unwrap().get_status() {
        //            PDBStatus::None => {
        //                match self.pending_breakpoints {
        //                    Some(ref mut x) => x.push(file_location.clone()),
        //                    None => {}
        //                };
        //                let f = future::lazy(move || {
        //                    let resp = serde_json::json!({"status":"PENDING"});
        //                    Ok(resp)
        //                });
        //                return Box::new(f);
        //            }
        //            _ => {}
        //        }
        //
        //        let (tx, rx) = mpsc::channel(1);
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .add_listener(Listener::Breakpoint, tx);
        //
        //        let f = rx
        //            .take(1)
        //            .into_future()
        //            .timeout(Duration::new(
        //                config
        //                    .lock()
        //                    .unwrap()
        //                    .get_config("BreakpointTimeout")
        //                    .unwrap() as u64,
        //                0,
        //            ))
        //            .map(move |event| match event.0.unwrap() {
        //                Event::BreakpointSet(_) => serde_json::json!({"status":"OK"}),
        //                _ => unreachable!(),
        //            })
        //            .map_err(|e| {
        //                eprintln!("Reading stdin error {:?}", e);
        //                io::Error::new(io::ErrorKind::Other, "Timed out setting breakpoint")
        //            });
        //
        //        let full_file_path = PathBuf::from(format!("{}", file_location.name));
        //        let full_file_name = full_file_path.canonicalize().unwrap();
        //        let stmt = format!(
        //            "break {}:{}\n",
        //            full_file_name.to_str().unwrap(),
        //            file_location.line_num
        //        );
        //
        //        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));
        //
        //        Box::new(f)
    }

    pub fn step_in(&mut self) {
        //        //match self.check_process_running() {
        //        //    Some(f) => return f,
        //        //    None => {}
        //        //};
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .write_stdin(Bytes::from("step\n"));
        //
        //        let f = future::lazy(move || {
        //            let resp = serde_json::json!({"status":"OK"});
        //            Ok(resp)
        //        });
        //
        //        Box::new(f)
    }

    pub fn step_over(&mut self) {
        //        //match self.check_process_running() {
        //        //    Some(f) => return f,
        //        //    None => {}
        //        //};
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .write_stdin(Bytes::from("next\n"));
        //
        //        let f = future::lazy(move || {
        //            let resp = serde_json::json!({"status":"OK"});
        //            Ok(resp)
        //        });
        //
        //        Box::new(f)
    }

    pub fn continue_(&mut self) {
        //        //match self.check_process_running() {
        //        //    Some(f) => return f,
        //        //    None => {}
        //        //};
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .write_stdin(Bytes::from("continue\n"));
        //
        //        let f = future::lazy(move || {
        //            let resp = serde_json::json!({"status":"OK"});
        //            Ok(resp)
        //        });
        //
        //        Box::new(f)
    }

    pub fn print(
        &mut self,
        variable: &Variable,
        //config: Arc<Mutex<Config>>,
    ) {
        //        //match self.check_process_running() {
        //        //    Some(f) => return f,
        //        //    None => {}
        //        //};
        //
        //        let (tx, rx) = mpsc::channel(1);
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .set_status(PDBStatus::Printing(variable.clone()));
        //
        //        self.process
        //            .lock()
        //            .unwrap()
        //            .add_listener(Listener::PrintVariable, tx);
        //
        //        let f = rx
        //            .take(1)
        //            .into_future()
        //            .timeout(Duration::new(
        //                config
        //                    .lock()
        //                    .unwrap()
        //                    .get_config("PrintVariableTimeout")
        //                    .unwrap() as u64,
        //                0,
        //            ))
        //            .map(move |event| match event.0.unwrap() {
        //                Event::PrintVariable(variable, value) => serde_json::json!({
        //                    "status": "OK",
        //                    "variable": variable.name,
        //                    "value": value,
        //                }),
        //                _ => unreachable!(),
        //            })
        //            .map_err(|e| {
        //                eprintln!("Reading stdin error {:?}", e);
        //                io::Error::new(io::ErrorKind::Other, "Timed out printing variable")
        //            });
        //
        //        let stmt = format!("print({})\n", variable.name);
        //
        //        self.process.lock().unwrap().write_stdin(Bytes::from(stmt));
        //
        //        Box::new(f)
    }
}
