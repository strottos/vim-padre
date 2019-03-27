//! lldb client debugger

use std::io;
use std::io::{BufRead};
use std::sync::{Arc, Condvar, mpsc, Mutex};
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;

use crate::request::{RequestError, Response};
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use regex::Regex;

mod lldb_process;

const TIMEOUT: u64 = 60000;

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    None,
    NoProcess,
    Error,
    ProcessStarted,
    Breakpoint,
    BreakpointPending,
    StepIn,
    StepOver,
    Continue,
    Variable,
    VariableNotFound,
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

        self.process.add_sender(Some(tx.clone()));
        self.sender = Some(tx);

        // Kick off lldb
        self.process.start_process(debugger_command, run_command, rx);

        let sender = self.sender.clone().unwrap();

        sender.send("settings set stop-line-count-after 0\n".to_string()).unwrap();
        sender.send("settings set stop-line-count-before 0\n".to_string()).unwrap();
        sender.send("settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n".to_string()).unwrap();

        // Send stdin to process
        thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap() + "\n";
                sender.send(line).unwrap();
            }
        });

        // TODO: Check listener for started.
        self.started = true;
    }

    fn has_started(&self) -> bool {
        self.started
    }

    fn stop(&self) {
        self.sender.clone().unwrap().send("quit\n".to_string()).expect("Can't communicate with LLDB");
    }

    fn run(&mut self) -> Result<Response<json::object::Object>, RequestError> {
        let (_, _) = self.check_response("break set --name main\n".to_string());

        let (_, args) = self.check_response("process launch\n".to_string());

        let mut ret = json::object::Object::new();
        ret.insert("pid", json::from(args.get(0).unwrap().to_string()));

        Ok(Response::OK(ret))
    }

    fn breakpoint(&mut self, file: String, line_num: u32) -> Result<Response<json::object::Object>, RequestError> {
        let (status, _) = self.check_response(format!("break set --file {} --line {}\n", file, line_num));

        match status {
            LLDBStatus::Breakpoint => Ok(Response::OK(json::object::Object::new())),
            LLDBStatus::BreakpointPending => Ok(Response::PENDING(json::object::Object::new())),
            _ => panic!("Didn't get a breakpoint response"),
        }
    }

    fn step_in(&mut self) -> Result<Response<json::object::Object>, RequestError> {
        let (status, _) = self.check_response("thread step-in\n".to_string());
        match status {
            LLDBStatus::StepIn => Ok(Response::OK(json::object::Object::new())),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn step_over(&mut self) -> Result<Response<json::object::Object>, RequestError> {
        let (status, _) = self.check_response("thread step-over\n".to_string());
        match status {
            LLDBStatus::StepOver => Ok(Response::OK(json::object::Object::new())),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn continue_on(&mut self) -> Result<Response<json::object::Object>, RequestError> {
        let (status, _) = self.check_response("thread continue\n".to_string());
        match status {
            LLDBStatus::Continue => Ok(Response::OK(json::object::Object::new())),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn print(&mut self, variable: String) -> Result<Response<json::object::Object>, RequestError> {
        match self.write_variable(&variable) {
            Ok(s) => return Ok(Response::OK(s)),
            Err(err) => return Err(err),
        };
    }
}

impl LLDB {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> LLDB {
        let process_notifier_clone = notifier.clone();
        let listener = Arc::new((Mutex::new((LLDBStatus::None, vec!())), Condvar::new()));
        let listener_process = listener.clone();
        LLDB {
            notifier: notifier,
            started: false,
            process: lldb_process::LLDBProcess::new(
                process_notifier_clone,
                listener_process,
                None,
            ),
            listener: listener,
            sender: None,
        }
    }

    fn check_response(&self, msg: String) -> (LLDBStatus, Vec<String>) {
        // Reset the current status
        let &(ref lock, ref cvar) = &*self.listener;
        let mut started = lock.lock().unwrap();
        *started = (LLDBStatus::None, vec!());

        // Send the request
        self.sender.clone().unwrap().send(msg.clone()).expect("Can't communicate with LLDB");

        // Check for the status change
        let result = cvar.wait_timeout(started, Duration::from_millis(TIMEOUT)).unwrap();
        started = result.0;

        match started.0 {
            LLDBStatus::None => {
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::CRITICAL,
                             format!("Timed out waiting for condition: {}", &msg));
                println!("Timed out waiting for condition: {}", msg);
                return (LLDBStatus::None, vec!());
            },
            _ => {},
        };

        let status = started.0.clone();
        let args = started.1.clone();

        (status, args)
    }

    fn throw_empty_error(&self) -> Result<Response<json::object::Object>, RequestError> {
        Err(RequestError::new("".to_string(), "".to_string()))
    }

    fn is_pointer_or_reference(&self, variable_type: &str) -> bool {
        lazy_static! {
            static ref RE_RUST_IS_POINTER_OR_REFERENCE: Regex = Regex::new("^&*.* \\*$").unwrap();
        }

        for _ in RE_RUST_IS_POINTER_OR_REFERENCE.captures_iter(variable_type) {
            return true;
        }

        false
    }

    fn write_variable(&self,
                      variable: &str) -> Result<json::object::Object, RequestError> {
        let (status, args) = self.check_response(format!("frame variable {}\n", variable));

        match status {
            LLDBStatus::Variable => {},
            LLDBStatus::VariableNotFound => {
                self.notifier.lock()
                             .unwrap()
                             .log_msg(LogLevel::WARN,
                                      format!("variable '{}' doesn't exist here", variable));
                return Err(RequestError::new("".to_string(), "".to_string()));
            },
            LLDBStatus::Error => {
                let err_msg = format!("{}", args.get(0).unwrap());
                return Err(RequestError::new(err_msg, "".to_string()));
            }
            LLDBStatus::NoProcess => {return Err(RequestError::new("".to_string(), "".to_string()));}
            _ => panic!("Shouldn't get here")
        }

        let variable = args.get(0).unwrap();
        let variable_type = args.get(2).unwrap();

        let mut ret = json::object::Object::new();
        ret.insert("variable", json::from(variable.to_string()));
        ret.insert("type", json::from(variable_type.to_string()));

        let variable_value = args.get(1).unwrap();
        ret.insert("value", json::from(variable_value.to_string()));

        if self.is_pointer_or_reference(variable_type) {
            let variable = format!("*{}", variable);
            let variable_deref = match self.write_variable(&variable) {
                Ok(s) => s,
                Err(err) => return Err(err),
            };
            ret.insert("deref", json::from(variable_deref));
        }

        Ok(ret)
    }
}
