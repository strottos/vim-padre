//! lldb client process

use std::io;
use std::io::{Read, Write};
use std::thread;
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::Receiver;

use crate::notifier::Notifier;
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
        let process = Command::new(debugger_command)
                              .args(run_command)
                              .stdin(Stdio::piped())
                              .stdout(Stdio::piped())
                              .spawn()
                              .unwrap();

        let mut process_stdout = process.stdout;
        let notifier = self.notifier.clone();

        let listener = self.listener.clone();

        thread::spawn(move || {
            match process_stdout.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        out.read(&mut buffer);
                        let data = String::from_utf8_lossy(&buffer[..]);

                        print!("{}", data);

                        analyse_data(data.into_owned().as_str(), &notifier, &listener);

                        io::stdout().flush().unwrap();
                    }
                }
                None => panic!("BALLS"),
            }
        });

        let mut process_stdin = process.stdin;

        thread::spawn(move || {
            loop {
                process_stdin.as_mut().unwrap().write(receiver.recv().unwrap().as_bytes());
            }
        });
    }
}

fn analyse_data(data: &str, notifier: &Arc<Mutex<Notifier>>, listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>) {
    lazy_static! {
        static ref RE_BREAKPOINT: Regex = Regex::new("^Breakpoint (\\d+): where = \\S+`\\S+ \\+ \\d+ at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d: \\S+`\\S.* at (\\S+):(\\d+)$").unwrap();
        static ref RE_PROCESS_EXITED: Regex = Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
        static ref RE_PRINTED_VARIABLE: Regex = Regex::new("^\\((\\S+)\\) (\\S+) = (.*)$").unwrap();
    }

    for line in data.split("\n") {
        for cap in RE_BREAKPOINT.captures_iter(line) {
            notifier.lock().unwrap().breakpoint_set(
                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
            send_listener(listener, LLDBStatus::BREAKPOINT, vec!());
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            notifier.lock().unwrap().jump_to_position(
                cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        }

        for cap in RE_PROCESS_EXITED.captures_iter(line) {
            notifier.lock().unwrap().signal_exited(
                cap[1].parse::<u32>().unwrap(), cap[2].parse::<u8>().unwrap());
        }

        for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
            let variable_type = match &cap[1] {
                "int" => "number",
                _ => "other",
            }.to_string();
            let variable_name = cap[2].to_string();
            let variable_value = cap[3].to_string();
            let args = vec!(variable_name, variable_value, variable_type);

            send_listener(listener, LLDBStatus::VARIABLE, args);
        }
    }
}

fn send_listener(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>, lldb_status: LLDBStatus, args: Vec<String>) {
    let &(ref lock, ref cvar) = &**listener;
    let mut started = lock.lock().unwrap();
    *started = (lldb_status, args);
    cvar.notify_one();
}
