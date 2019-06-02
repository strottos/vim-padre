//! lldb client debugger

use std::io;
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use bytes::Bytes;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug, Clone)]
pub enum LLDBStatus {
    None,
    NoProcess,
    Error,
    // (PID)
    ProcessStarted(u32),
    // (PID, Exit code)
    ProcessExited(u32, u32),
    // (File name, line number)
    Breakpoint(String, u32),
    // (File name, line number)
    JumpToPosition(String, u32),
    UnknownPosition,
    BreakpointPending,
    StepIn,
    StepOver,
    Continue,
    Variable,
    VariableNotFound,
}

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    lldb_in_tx: Option<Sender<Bytes>>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            lldb_in_tx: None,
        }
    }

    fn send_lldb(&mut self, bytes: Bytes) {
        assert!(!self.lldb_in_tx.is_none());
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();
        tokio::spawn(
            lldb_in_tx
                .send(bytes)
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );
    }

}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {
        if !Path::new(&self.run_cmd[0]).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]),
            );
            println!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]);
            exit(1);
        }

        let (lldb_in_tx, lldb_in_rx) = mpsc::channel(1);
        let (lldb_out_tx, lldb_out_rx) = mpsc::channel(32);

        self.lldb_in_tx = Some(lldb_in_tx);

        let mut cmd = vec![self.debugger_cmd.clone(), "--".to_string()];
        cmd.extend(self.run_cmd.clone());
        spawn_process(cmd, lldb_in_rx, lldb_out_tx);

        self.send_lldb(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]));
        self.send_lldb(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]));
        self.send_lldb(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]));

        // This is the preferred method but doesn't seem to work with current tokio
        // Example here states we need a separate thread: https://github.com/tokio-rs/tokio/blob/master/tokio/examples/connect.rs
        //
        //        let input = FramedRead::new(stdin(), LinesCodec::new());
        //
        //        tokio::spawn(
        //            input
        //                .for_each(|req| {
        //                    println!("{:?}", req);
        //                    Ok(())
        //                })
        //                .map(|_| ())
        //                .map_err(|e| panic!("io error = {:?}", e))
        //        );

        let notifier = self.notifier.clone();

        tokio::spawn(
            lldb_out_rx.for_each(move |output| {
                match analyse_lldb_output(output) {
                    LLDBStatus::ProcessStarted(pid) => {
                        println!("Process started {}", pid);
                    },
                    LLDBStatus::ProcessExited(pid, exit_code) => {
                        notifier.lock().unwrap().signal_exited(pid, exit_code);
                        println!("Process exited {}", exit_code);
                    },
                    LLDBStatus::Breakpoint(file, line) => {
                        notifier.lock().unwrap().breakpoint_set(file, line);
                    },
                    LLDBStatus::JumpToPosition(file, line) => {
                        notifier.lock().unwrap().jump_to_position(file, line);
                    },
                    LLDBStatus::None => {}
                    _ => panic!("Uh oh"),
                }
                Ok(())
            })
            .map_err(|e| panic!("Error receiving from lldb: {}", e))
        );

        let mut lldb_in_tx = self.lldb_in_tx.clone().unwrap();

        thread::spawn(|| {
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                let bytes = Bytes::from(buf);
                lldb_in_tx = match lldb_in_tx.send(bytes).wait() {
                    Ok(tx) => tx,
                    Err(_) => break,
                };
            }
        });

        // TODO: Send when actually started
        self.notifier.lock().unwrap().signal_started();
    }
}

fn analyse_lldb_output(output: Bytes) -> LLDBStatus {
    let data = String::from_utf8_lossy(&output[..]);
    let data = data.trim_matches(char::from(0));

    lazy_static! {
        static ref RE_BREAKPOINT: Regex = Regex::new("Breakpoint (\\d+): where = .* at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
        static ref RE_STOPPED_AT_POSITION: Regex = Regex::new(" *frame #\\d.*$").unwrap();
        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
        static ref RE_PROCESS_STARTED: Regex = Regex::new("Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
        static ref RE_PROCESS_EXITED: Regex = Regex::new("Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
    }

    for line in data.split("\r\n") {
        for cap in RE_PROCESS_STARTED.captures_iter(line) {
            return LLDBStatus::ProcessStarted(cap[1].parse::<u32>().unwrap());
        }

        for cap in RE_PROCESS_EXITED.captures_iter(line) {
            return LLDBStatus::ProcessExited(cap[1].parse::<u32>().unwrap(), cap[2].parse::<u32>().unwrap());
        }

        for cap in RE_BREAKPOINT.captures_iter(line) {
            return LLDBStatus::Breakpoint(cap[2].to_string(), cap[3].parse::<u32>().unwrap());
        }

        for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                return LLDBStatus::JumpToPosition(cap[1].to_string(), cap[2].parse::<u32>().unwrap());
            }

            return LLDBStatus::UnknownPosition;
        }
    }

    LLDBStatus::None
}
