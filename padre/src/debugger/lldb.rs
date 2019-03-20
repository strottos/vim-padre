//! lldb client debugger

use std::io;
use std::io::{BufRead};
use std::process::exit;
use std::sync::{Arc, Condvar, mpsc, Mutex};
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::Duration;

use crate::request::{RequestError, Response};
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use regex::Regex;

mod lldb_process;

const TIMEOUT: u64 = 5000;

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    None,
    NoProcess,
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
        sender.send("settings set frame-format frame #${frame.index}: {${module.file.basename}{`${function.name-with-args}{${frame.no-debug}${function.pc-offset}}}}{ at ${line.file.fullpath}:${line.number}}\\n\n".to_string()).unwrap();

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

    fn run(&mut self) -> Result<Response<Option<String>>, RequestError> {
        let (_, _) = self.check_response("break set --name main\n".to_string());

        let (_, args) = self.check_response("process launch\n".to_string());

        let ret = format!("pid={}", args.get(0).unwrap());

        Ok(Response::OK(Some(ret)))
    }

    fn breakpoint(&mut self, file: String, line_num: u32) -> Result<Response<Option<String>>, RequestError> {
        let (status, _) = self.check_response(format!("break set --file {} --line {}\n", file, line_num));

        match status {
            LLDBStatus::Breakpoint => Ok(Response::OK(None)),
            LLDBStatus::BreakpointPending => Ok(Response::PENDING(None)),
            _ => panic!("Didn't get a breakpoint response"),
        }
    }

    fn step_in(&mut self) -> Result<Response<Option<String>>, RequestError> {
        let (status, _) = self.check_response("thread step-in\n".to_string());
        match status {
            LLDBStatus::StepIn => Ok(Response::OK(None)),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn step_over(&mut self) -> Result<Response<Option<String>>, RequestError> {
        let (status, _) = self.check_response("thread step-over\n".to_string());
        match status {
            LLDBStatus::StepOver => Ok(Response::OK(None)),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn continue_on(&mut self) -> Result<Response<Option<String>>, RequestError> {
        let (status, _) = self.check_response("thread continue\n".to_string());
        match status {
            LLDBStatus::Continue => Ok(Response::OK(None)),
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Didn't get a step-in response"),
        }
    }

    fn print(&mut self, variable: String) -> Result<Response<Option<String>>, RequestError> {
        let (status, args) = self.check_response(format!("frame variable {}\n", variable));

        match status {
            LLDBStatus::Variable => {},
            LLDBStatus::VariableNotFound => {
                self.notifier.lock()
                             .unwrap()
                             .log_msg(LogLevel::WARN,
                                      format!("variable '{}' doesn't exist here", variable));
                return self.throw_empty_error();
            },
            LLDBStatus::NoProcess => {return self.throw_empty_error();}
            _ => panic!("Shouldn't get here")
        }

        let variable = args.get(0).unwrap();
        let variable_value = args.get(1).unwrap();
        let variable_type = args.get(2).unwrap();

        if self.is_pointer_or_reference(variable_type) {
            return self.analyse_pointer(variable, variable_type)
        }

        let ret = format!("variable={} value=\"{}\" type=\"{}\"",
                          args.get(0).unwrap(),
                          variable_value,
                          variable_type);

        Ok(Response::OK(Some(ret)))
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
                exit(1);
            },
            _ => {},
        };

        let status = started.0.clone();
        let args = started.1.clone();

        (status, args)
    }

    fn throw_empty_error(&self) -> Result<Response<Option<String>>, RequestError> {
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

    fn analyse_pointer(&self,
                       variable: &str,
                       variable_type: &str) -> Result<Response<Option<String>>, RequestError> {
        let (status, args) = self.check_response(format!("frame variable *{}\n", variable));

        match status {
            LLDBStatus::Variable => {},
            _ => panic!("Shouldn't get here")
        }

        let mut obj = json::object::Object::new();
        obj.insert("type", json::from(variable_type.to_string()));
        obj.insert("deref", json::from(args.get(1).unwrap().to_string()));

        let variable_value = json::stringify(obj);
        let variable_type = "JSON";
        println!("{}", variable_value);

        let ret = json::stringify(format!("variable={} value=\"{}\" type=\"{}\"",
                                          variable,
                                          variable_value,
                                          variable_type));
        println!("{}", ret);

        Ok(Response::OK(Some(ret)))
    }
}
