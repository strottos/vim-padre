//! lldb client process

use std::io;
use std::io::{Read, Write};
use std::thread;
use std::path::Path;
use std::process::{Command, exit, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::Receiver;

use crate::notifier::{LogLevel, Notifier};
use crate::debugger::lldb::LLDBStatus;

use regex::Regex;

pub struct LLDBProcess {
    notifier: Arc<Mutex<Notifier>>,
    listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
}

impl LLDBProcess {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
    ) -> LLDBProcess {
        LLDBProcess {
            notifier: notifier,
            listener: listener,
        }
    }

    pub fn start_process(
        &mut self,
        debugger_command: String,
        run_command: &Vec<String>,
        receiver: Receiver<String>,
    ) {
        if !Path::new(&run_command[0]).exists() {
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::CRITICAL,
                         format!("Can't spawn LLDB as {} does not exist", run_command[0]));
            println!("Can't spawn LLDB as {} does not exist", run_command[0]);
            exit(1);
        }

        let process = Command::new(debugger_command)
                              .args(run_command)
                              .stdin(Stdio::piped())
                              .stdout(Stdio::piped())
                              .spawn()
                              .unwrap();

        let mut process_stdout = process.stdout;
        let notifier = self.notifier.clone();
        let notifier_err = self.notifier.clone();

        let listener = self.listener.clone();

        thread::spawn(move || {
            match process_stdout.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        match out.read(&mut buffer) {
                            Err(err) => {
                                notifier_err.lock()
                                            .unwrap()
                                            .log_msg(LogLevel::CRITICAL,
                                                     format!("Can't read from LLDB: {}", err));
                                println!("Can't read from LLDB: {}", err);
                                exit(1);
                            },
                            _ => {},
                        };
                        let data = String::from_utf8_lossy(&buffer[..]);
                        let data = data.trim_matches(char::from(0));

                        print!("{}", &data);

                        analyse_data(data, &notifier, &listener);

                        io::stdout().flush().unwrap();
                    }
                }
                None => {
                    notifier_err.lock()
                                .unwrap()
                                .log_msg(LogLevel::CRITICAL,
                                         "Can't read from LLDB".to_string());
                    println!("Can't read from LLDB");
                    exit(1);
                }
            }
        });

        let mut process_stdin = process.stdin;
        let notifier_err = self.notifier.clone();

        thread::spawn(move || {
            loop {
                match process_stdin.as_mut().unwrap().write(receiver.recv().unwrap().as_bytes()) {
                    Err(err) => {
                        notifier_err.lock()
                                    .unwrap()
                                    .log_msg(LogLevel::CRITICAL,
                                             format!("Can't read from PADRE stdin: {}", err));
                        println!("Can't read from PADRE stdin: {}", err);
                        exit(1);
                    },
                    _ => {}
                };
            }
        });
    }
}

fn analyse_data(data: &str, notifier: &Arc<Mutex<Notifier>>, listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>) {
    lazy_static! {
        static ref RE_BREAKPOINT: Regex = Regex::new("^Breakpoint (\\d+): where = \\S+`\\S+ \\+ \\d+ at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
        static ref RE_BREAKPOINT_PENDING: Regex = Regex::new("^Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
        static ref RE_BREAKPOINT_MULTI: Regex = Regex::new("^Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d: \\S+`\\S.* at (\\S+):(\\d+)$").unwrap();
        static ref RE_STEP_IN: Regex = Regex::new("^\\* .* stop reason = step in$").unwrap();
        static ref RE_STEP_OVER: Regex = Regex::new("^\\* .* stop reason = step over$").unwrap();
        static ref RE_CONTINUE: Regex = Regex::new("^Process (\\d+) resuming$").unwrap();
        static ref RE_PRINTED_VARIABLE: Regex = Regex::new("^\\((\\S+)\\) (\\S+) = (.*)$").unwrap();
        static ref RE_PROCESS_STARTED: Regex = Regex::new("^Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
        static ref RE_PROCESS_EXITED: Regex = Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
    }

    for line in data.split("\n") {
        for cap in RE_BREAKPOINT.captures_iter(line) {
            notifier.lock().unwrap().breakpoint_set(
                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
            send_listener(listener, LLDBStatus::Breakpoint, vec!());
        }

        for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
            send_listener(listener, LLDBStatus::BreakpointPending, vec!());
        }

        for _ in RE_BREAKPOINT_MULTI.captures_iter(line) {
            send_listener(listener, LLDBStatus::Breakpoint, vec!());
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            notifier.lock().unwrap().jump_to_position(
                cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        }

        for _ in RE_STEP_IN.captures_iter(line) {
            send_listener(listener, LLDBStatus::StepIn, vec!());
        }

        for _ in RE_STEP_OVER.captures_iter(line) {
            send_listener(listener, LLDBStatus::StepOver, vec!());
        }

        for _ in RE_CONTINUE.captures_iter(line) {
            send_listener(listener, LLDBStatus::Continue, vec!());
        }

        for cap in RE_PROCESS_STARTED.captures_iter(line) {
            let args = vec!(cap[1].to_string());

            send_listener(listener, LLDBStatus::ProcessStarted, args);
        }

        for cap in RE_PROCESS_EXITED.captures_iter(line) {
            notifier.lock().unwrap().signal_exited(
                cap[1].parse::<u32>().unwrap(), cap[2].parse::<u8>().unwrap());
        }

        for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
            let variable_type = match &cap[1] {
                "int" => "number",
                "(&str)" => "string",
                _ => panic!("Code this: {}", line),
            }.to_string();
            let variable_name = cap[2].to_string();
            let variable_value = cap[3].to_string();
            let args = vec!(variable_name, variable_value, variable_type);

            send_listener(listener, LLDBStatus::Variable, args);
        }
    }
}

fn send_listener(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>, lldb_status: LLDBStatus, args: Vec<String>) {
    let &(ref lock, ref cvar) = &**listener;
    let mut started = lock.lock().unwrap();
    *started = (lldb_status, args);
    cvar.notify_one();
}
