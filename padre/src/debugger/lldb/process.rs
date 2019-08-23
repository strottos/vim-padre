//! lldb process handler
//!
//! This module performs the basic setup of and interfacing with LLDB. It will
//! analyse the output of the text and work out what is happening then.
//!
//! There are two processes generally to be concerned with, firstly there is the
//! LLDB process, secondly there is the program you're debugging. We keep track
//! of the status of each of these things throughout and act appropriately based
//! on what status we are currently in and the instructions we have received.

use std::collections::HashMap;
use std::io::{self, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::debugger::{FileLocation, Variable};
use crate::notifier::{breakpoint_set, jump_to_position, log_msg, signal_exited, LogLevel};
use crate::util::read_output;

use bytes::Bytes;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};
use tokio_process::{Child, ChildStderr, ChildStdin, ChildStdout, CommandExt};

/// LLDB can be in one of four statuses:
/// - None: This means that LLDB is not running, generally this is not
///   very interesting.
/// - Working: This means LLDB is in a status of processing and will not be
///   responsive to commands sent to it.
/// - Listening: This means LLDB is currently paused and awaiting further
///   instructions
/// - Exiting: This means LLDB is in a status of shutting down
#[derive(Debug, Clone)]
enum LLDBStatus {
    None,
    Listening,
    Working,
    Exiting,
}

/// You can register to listen for one of the following events:
/// - LLDBLaunched: LLDB has started up initially
/// - ProcessLaunched: LLDB has launched a process for debugging
/// - ProcessExited: The process spawned by LLDB has exited
/// - ProcessPaused: The process has stopped
/// - Breakpoint: A breakpoint event has happened
/// - PrintVariable: A variable has been requested to print and this is the response
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum LLDBListener {
    LLDBLaunched,
    ProcessLaunched,
    ProcessExited,
    ProcessPaused,
    Breakpoint,
    PrintVariable,
}

/// An LLDB event is something that
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum LLDBEvent {
    LLDBLaunched,
    // (PID)
    ProcessLaunched(u64),
    // (PID, Exit code)
    ProcessExited(u64, i64),
    // (File name, line number)
    BreakpointSet(FileLocation),
    BreakpointMultiple,
    BreakpointPending,
    // (File name, line number)
    JumpToPosition(FileLocation),
    UnknownPosition,
    // (type, variable, value)
    PrintVariable(Variable),
    VariableNotFound,
}

#[derive(Debug)]
pub struct LLDBProcess {
    debugger_cmd: String,
    run_cmd: Vec<String>,
    lldb_process: Option<Child>,
    lldb_stdin_tx: Option<Sender<Bytes>>,
    lldb_analyser: Arc<Mutex<LLDBAnalyser>>,
}

impl LLDBProcess {
    /// Create a new LLDBProcess
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        LLDBProcess {
            debugger_cmd,
            run_cmd,
            lldb_process: None,
            lldb_stdin_tx: None,
            lldb_analyser: Arc::new(Mutex::new(LLDBAnalyser::new())),
        }
    }

    /// Setup LLDB
    ///
    /// Includes spawning the LLDB process and all the relevant stdio handlers. In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read stdout and stderr
    /// - Sets up a thread to read stdin and forward it onto LLDB stdin.
    pub fn setup(&mut self) {
        let mut lldb_process = Command::new(&self.debugger_cmd)
            .arg("--")
            .args(&self.run_cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_async()
            .expect("Failed to spawn LLDB");

        self.setup_stdout(
            lldb_process
                .stdout()
                .take()
                .expect("LLDB process did not have a handle to stdout"),
        );
        self.setup_stderr(
            lldb_process
                .stderr()
                .take()
                .expect("LLDB process did not have a handle to stderr"),
        );
        self.setup_stdin(
            lldb_process
                .stdin()
                .take()
                .expect("LLDB process did not have a handle to stdin"),
        );

        self.lldb_process = Some(lldb_process);
    }

    pub fn teardown(&mut self) {
        self.lldb_process = None;
    }

    /// Send a message to write to stdin
    pub fn write_stdin(&mut self, bytes: Bytes) {
        let tx = self.lldb_stdin_tx.clone();
        let lldb_analyser = self.lldb_analyser.clone();
        tokio::spawn(
            tx.clone()
                .unwrap()
                .send(bytes)
                .map(move |_| {
                    lldb_analyser.lock().unwrap().set_working();
                })
                .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        );
    }

    pub fn add_listener(&mut self, kind: LLDBListener, sender: Sender<LLDBEvent>) {
        self.lldb_analyser
            .lock()
            .unwrap()
            .add_listener(kind, sender);
    }

    /// Perform setup of listening and forwarding of stdin and setup ability to send to
    /// LLDB stdin.
    fn setup_stdin(&mut self, mut stdin: ChildStdin) {
        let (stdin_tx, stdin_rx) = mpsc::channel(1);
        let mut tx = stdin_tx.clone();

        thread::spawn(move || {
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                tx = match tx.send(Bytes::from(buf)).wait() {
                    Ok(tx) => tx,
                    Err(_) => break,
                };
            }
        });

        // Current implementation needs a kick, this is all liable to change with
        // upcoming versions of tokio anyway so living with it for now.
        match stdin.write(&[13]) {
            Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => {}
            _ => unreachable!(),
        }

        tokio::spawn(
            stdin_rx
                .for_each(move |text| {
                    match stdin.write(&text) {
                        Ok(_) => {}
                        Err(e) => {
                            panic!("Writing LLDB stdin err e: {}", e);
                        }
                    };
                    Ok(())
                })
                .map_err(|e| {
                    eprintln!("Reading stdin error {:?}", e);
                }),
        );

        self.lldb_stdin_tx = Some(stdin_tx);
    }

    /// Perform setup of reading LLDB stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let lldb_analyser = self.lldb_analyser.clone();
        tokio::spawn(
            read_output(BufReader::new(stdout))
                .for_each(move |text| {
                    print!("{}", text);
                    lldb_analyser.lock().unwrap().analyse_text(&text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading LLDB stdout: {}", e)),
        );
    }

    /// Perform setup of reading LLDB stderr, analysing it and writing it back to stdout.
    fn setup_stderr(&mut self, stderr: ChildStderr) {
        tokio::spawn(
            read_output(BufReader::new(stderr))
                .for_each(|text| {
                    eprint!("STDERR: {}", text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading LLDB stderr: {}", e)),
        );
    }
}

#[derive(Debug)]
pub struct LLDBAnalyser {
    text: String,
    lldb_status: LLDBStatus,
    listeners: HashMap<LLDBListener, Sender<LLDBEvent>>,
}

impl LLDBAnalyser {
    pub fn new() -> Self {
        LLDBAnalyser {
            text: "".to_string(),
            lldb_status: LLDBStatus::None,
            listeners: HashMap::new(),
        }
    }

    pub fn add_listener(&mut self, kind: LLDBListener, sender: Sender<LLDBEvent>) {
        self.listeners.insert(kind, sender);
    }

    pub fn analyse_text(&mut self, s: &str) {
        self.text.push_str(s);

        lazy_static! {
            static ref RE_LLDB_STARTED: Regex =
                Regex::new("^Current executable set to '.*' (.*)\\.$").unwrap();
            static ref RE_PROCESS_STARTED: Regex =
                Regex::new("^Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$")
                    .unwrap();
            static ref RE_BREAKPOINT: Regex = Regex::new(
                "Breakpoint (\\d+): where = .* at (.*):(\\d+):\\d+, address = 0x[0-9a-f]*$"
            )
            .unwrap();
            static ref RE_BREAKPOINT_2: Regex =
                Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+), address = 0x[0-9a-f]*$")
                    .unwrap();
            static ref RE_BREAKPOINT_MULTIPLE: Regex =
                Regex::new("Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
            static ref RE_BREAKPOINT_PENDING: Regex =
                Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
            static ref RE_STOPPED_AT_POSITION: Regex = Regex::new(" *frame #\\d.*$").unwrap();
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
        }

        let s = self.text.clone();

        for line in s.split("\n") {
            for _ in RE_LLDB_STARTED.captures_iter(line) {
                self.lldb_started();
            }

            for cap in RE_PROCESS_STARTED.captures_iter(line) {
                let pid = cap[1].parse::<u64>().unwrap();
                self.process_started(pid);
            }

            for cap in RE_PROCESS_EXITED.captures_iter(line) {
                let pid = cap[1].parse::<u64>().unwrap();
                let exit_code = cap[2].parse::<i64>().unwrap();
                self.process_exited(pid, exit_code);
            }

            let mut found_breakpoint = false;

            for cap in RE_BREAKPOINT.captures_iter(line) {
                found_breakpoint = true;
                let file = cap[2].to_string();
                let line = cap[3].parse::<u64>().unwrap();
                self.found_breakpoint(file, line);
            }

            if !found_breakpoint {
                for cap in RE_BREAKPOINT_2.captures_iter(line) {
                    found_breakpoint = true;
                    let file = cap[2].to_string();
                    let line = cap[3].parse::<u64>().unwrap();
                    self.found_breakpoint(file, line);
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_MULTIPLE.captures_iter(line) {
                    found_breakpoint = true;
                    self.found_multiple_breakpoints();
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
                    self.found_pending_breakpoint();
                }
            }

            for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
                self.lldb_status = LLDBStatus::Listening;

                let mut found = false;
                for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                    found = true;
                    let file = cap[1].to_string();
                    let line = cap[2].parse::<u64>().unwrap();
                    self.jump_to_position(file, line);
                }

                if !found {
                    self.jump_to_unknown_position();
                }
            }
        }

        self.clear_analyser();
    }

    pub fn set_working(&mut self) {
        self.lldb_status = LLDBStatus::Working;
    }

    fn clear_analyser(&mut self) {
        self.text = "".to_string();
    }

    fn lldb_started(&mut self) {
        self.lldb_status = LLDBStatus::Working;
        match self.listeners.remove(&LLDBListener::LLDBLaunched) {
            Some(listener) => {
                listener.send(LLDBEvent::LLDBLaunched).wait().unwrap();
            }
            None => {}
        }
    }

    fn process_started(&mut self, pid: u64) {
        self.lldb_status = LLDBStatus::Working;
        match self.listeners.remove(&LLDBListener::ProcessLaunched) {
            Some(listener) => {
                listener
                    .send(LLDBEvent::ProcessLaunched(pid))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn process_exited(&mut self, pid: u64, exit_code: i64) {
        self.lldb_status = LLDBStatus::Working;
        signal_exited(pid, exit_code);
        match self.listeners.remove(&LLDBListener::ProcessExited) {
            Some(listener) => {
                listener
                    .send(LLDBEvent::ProcessExited(pid, exit_code))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn found_breakpoint(&mut self, file: String, line: u64) {
        breakpoint_set(&file, line);
        let file_location = FileLocation::new(file, line);
        match self.listeners.remove(&LLDBListener::Breakpoint) {
            Some(listener) => {
                listener
                    .send(LLDBEvent::BreakpointSet(file_location))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn found_multiple_breakpoints(&mut self) {
        match self.listeners.remove(&LLDBListener::Breakpoint) {
            Some(listener) => {
                listener.send(LLDBEvent::BreakpointMultiple).wait().unwrap();
            }
            None => {}
        }
    }

    fn found_pending_breakpoint(&mut self) {
        match self.listeners.remove(&LLDBListener::Breakpoint) {
            Some(listener) => {
                listener.send(LLDBEvent::BreakpointPending).wait().unwrap();
            }
            None => {}
        }
    }

    fn jump_to_position(&mut self, file: String, line: u64) {
        jump_to_position(&file, line);
    }

    fn jump_to_unknown_position(&mut self) {
        log_msg(LogLevel::WARN, "Stopped at unknown position");
    }
}
