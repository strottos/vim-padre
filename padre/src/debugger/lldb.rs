//! lldb client debugger

use std::convert::From;
use std::io;
use std::io::{BufRead};
use std::sync::{Arc, Condvar, mpsc, Mutex};
use std::sync::mpsc::SyncSender;
use std::thread;

use crate::request::{RequestError, Response};
use crate::debugger::Debugger;
use crate::notifier::Notifier;

mod lldb_process;

#[derive(Debug)]
pub enum LLDBStatus {
    NONE,
    BREAKPOINT,
    STEP_IN,
    STEP_OVER,
    VARIABLE,
}

pub struct LLDB {
    notifier: Arc<Mutex<Notifier>>,
    started: bool,
    process: lldb_process::LLDBProcess,
    listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
    sender: Option<SyncSender<String>>,
}

impl Debugger for LLDB {
    fn start(&mut self, debugger_command: String, run_command: &Vec<String>) {
        let (tx, rx) = mpsc::sync_channel(512);

        self.sender = Some(tx.clone());

        // Kick off lldb
        self.process.start_process(debugger_command, run_command, rx);

        tx.send("settings set stop-line-count-after 0\n".to_string()).unwrap();
        tx.send("settings set stop-line-count-before 0\n".to_string()).unwrap();
        tx.send("settings set frame-format frame #${frame.index}: {${module.file.basename}{`${function.name-with-args}{${frame.no-debug}${function.pc-offset}}}}{ at ${line.file.fullpath}:${line.number}}\\n\n".to_string()).unwrap();

        // Send stdin to process
        thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap() + "\n";
                tx.send(line).unwrap();
            }
        });

        // TODO: Check listener for started.
        self.started = true;
    }

    fn has_started(&self) -> bool {
        self.started
    }

    fn stop(&self) {
        println!("STOP");
    }

    fn run(&mut self) -> Result<Response<Option<String>>, RequestError> {
        self.sender.clone().unwrap().send("break set --name main\n".to_string()).expect("Can't communicate with LLDB");
        self.sender.clone().unwrap().send("process launch\n".to_string()).expect("Can't communicate with LLDB");
        Ok(Response::OK(None))
    }

    fn breakpoint(&mut self, file: String, line_num: u32) -> Result<Response<Option<String>>, RequestError> {
        // Check breakpoint comes back
        let &(ref lock, ref cvar) = &*self.listener;
        let mut started = lock.lock().unwrap();
        *started = (LLDBStatus::NONE, vec!());

        self.sender.clone().unwrap().send(format!("break set --file {} --line {}\n", file, line_num)).expect("Can't communicate with LLDB");

        // Check breakpoint comes back
        loop {
            match started.0 {
                LLDBStatus::NONE => {}
                _ => {break}
            };
            started = cvar.wait(started).unwrap();
        }

        Ok(Response::OK(None))
    }

    fn stepIn(&mut self) -> Result<Response<Option<String>>, RequestError> {
        self.sender.clone().unwrap().send("thread step-in\n".to_string()).expect("Can't communicate with LLDB");
        Ok(Response::OK(None))
    }

    fn stepOver(&mut self) -> Result<Response<Option<String>>, RequestError> {
        self.sender.clone().unwrap().send("thread step-over\n".to_string()).expect("Can't communicate with LLDB");
        Ok(Response::OK(None))
    }

    fn carryOn(&mut self) -> Result<Response<Option<String>>, RequestError> {
        self.sender.clone().unwrap().send("thread continue\n".to_string()).expect("Can't communicate with LLDB");
        Ok(Response::OK(None))
    }

    fn print(&mut self, variable: String) -> Result<Response<Option<String>>, RequestError> {
        // Check variable comes back
        let &(ref lock, ref cvar) = &*self.listener;
        let mut started = lock.lock().unwrap();
        *started = (LLDBStatus::NONE, vec!());

        self.sender.clone().unwrap().send(format!("frame variable {}\n", variable)).expect("Can't communicate with LLDB");

        loop {
            match started.0 {
                LLDBStatus::NONE => {}
                _ => {break}
            };
            started = cvar.wait(started).unwrap();
        }

        match started.0 {
            LLDBStatus::VARIABLE => {},
            _ => panic!("Shouldn't get here")
        }

        let args = &started.1;
        let ret = format!("variable={} value={} type={}",
                          args.get(0).unwrap(),
                          args.get(1).unwrap(),
                          args.get(2).unwrap());

        Ok(Response::OK(Some(ret)))
    }
}

impl LLDB {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> LLDB {
        let process_notifier_clone = notifier.clone();
        let listener = Arc::new((Mutex::new((LLDBStatus::NONE, vec!())), Condvar::new()));
        let listener_process = listener.clone();
        LLDB {
            notifier: notifier,
            started: false,
            process: lldb_process::LLDBProcess::new(
                process_notifier_clone,
                listener_process,
            ),
            listener: listener,
            sender: None,
        }
    }
}
