//! lldb client process

use std::io;
use std::io::{Read, Write};
use std::thread;
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::{SyncSender, Receiver};

use crate::notifier::Notifier;
use crate::debugger::lldb::{LLDBStop, LLDBError};

use regex::Regex;

pub struct LLDBProcess {
    notifier: Arc<Mutex<Notifier>>,
    stop_listener: Arc<(Mutex<Option<Result<LLDBStop, LLDBError>>>, Condvar)>,
}

impl LLDBProcess {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        stop_listener: Arc<(Mutex<Option<Result<LLDBStop, LLDBError>>>,
                            Condvar)>,
    ) -> LLDBProcess {
        LLDBProcess {
            notifier: notifier,
            stop_listener: stop_listener,
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

        thread::spawn(move || {
            match process_stdout.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        out.read(&mut buffer);
                        let data = String::from_utf8_lossy(&buffer[..]);

                        print!("{}", data);

                        analyse_data(data.into_owned().as_str(), &notifier);

                        io::stdout().flush().unwrap();
                    }
                }
                None => panic!("BALLS"),
            }
        });

        let mut process_stdin = process.stdin;

        process_stdin.as_mut().unwrap().write("settings set stop-line-count-after 0\n".as_bytes());
        process_stdin.as_mut().unwrap().write("settings set stop-line-count-before 0\n".as_bytes());
        process_stdin.as_mut().unwrap().write("settings set frame-format frame #${frame.index}: {${module.file.basename}{`${function.name-with-args}{${frame.no-debug}${function.pc-offset}}}}{ at ${line.file.fullpath}:${line.number}}\\n\n".as_bytes());

        thread::spawn(move || {
            loop {
                process_stdin.as_mut().unwrap().write(receiver.recv().unwrap().as_bytes());
            }
        });
    }
}

fn analyse_data(data: &str, notifier: &Arc<Mutex<Notifier>>) {
    lazy_static! {
        static ref RE_BREAKPOINT: Regex = Regex::new("^Breakpoint (\\d+): where = \\S+`\\S+ \\+ \\d+ at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d: \\S+`\\S.* at (\\S+):(\\d+)$").unwrap();
    }

    for line in data.split("\n") {
        for cap in RE_BREAKPOINT.captures_iter(line) {
            notifier.lock().unwrap().breakpoint_set(
                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            notifier.lock().unwrap().jump_to_position(
                cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        }
    }
}
