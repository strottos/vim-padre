//! lldb client process

use std::borrow::Cow;
use std::io;
use std::io::{BufRead, Read, Write};
use std::thread;
use std::process::{Command, Child, Stdio};
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};

use regex::Regex;

pub struct LLDBProcess {
    notifier: Arc<Mutex<Notifier>>,
}

impl LLDBProcess {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> LLDBProcess {
        LLDBProcess {
            notifier: notifier,
        }
    }

    pub fn start_process(&mut self, debugger_command: String, run_command: &Vec<String>) {
        let mut process = Command::new(debugger_command)
                                  .args(run_command)
                                  .stdin(Stdio::piped())
                                  .stdout(Stdio::piped())
                                  .spawn()
                                  .unwrap();

        let mut process_stdout = process.stdout;
        let notifier = self.notifier.clone();

        let handle1 = thread::spawn(move || {
            match process_stdout.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        out.read(&mut buffer);
                        let data = String::from_utf8_lossy(&buffer[..]);
                        println!("DATA");

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

        let handle2 = thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap() + "\n";
                process_stdin.as_mut().unwrap().write(line.as_bytes());
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
            println!("HERE {:?}", cap);
            notifier.lock().unwrap().breakpoint_set(
                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            println!("HERE {:?}", cap);
            notifier.lock().unwrap().jump_to_position(
                cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        }
    }
}
