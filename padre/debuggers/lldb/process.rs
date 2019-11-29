//! lldb process handler
//!
//! This module performs the basic setup of and interfacing with LLDB. It will
//! analyse the output of the text and work out what is happening then.

use std::collections::HashMap;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

use crate::debugger::{FileLocation, Variable};
use crate::notifier::{breakpoint_set, jump_to_position, log_msg, signal_exited, LogLevel};
use crate::util::{check_and_spawn_process, read_output}; //, setup_stdin};

use bytes::Bytes;
use regex::Regex;
use tokio::net::process::{Child, ChildStderr, ChildStdout};
use tokio::prelude::*;
use tokio::sync::mpsc::Sender;

/// You can register to listen for one of the following events:
/// - LLDBLaunched: LLDB has started up initially
/// - ProcessLaunched: LLDB has launched a process for debugging
/// - ProcessExited: The process spawned by LLDB has exited
/// - Breakpoint: A breakpoint event has happened
/// - PrintVariable: A variable has been requested to print and this is the response
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Listener {
    LLDBLaunched,
    ProcessLaunched,
    ProcessExited,
    Breakpoint,
    PrintVariable,
}

/// An LLDB event is something that can be registered for being listened to and can be triggered
/// when these events occur such that the listener is informed of them and passed some details
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Event {
    LLDBLaunched,
    // (PID)
    ProcessLaunched(u64),
    // (PID, Exit code)
    ProcessExited(u64, i64),
    BreakpointSet(FileLocation),
    BreakpointMultiple,
    BreakpointPending,
    PrintVariable(Variable, VariableValue),
    VariableNotFound(Variable),
}

/// The value of a variable
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VariableValue {
    type_: String,
    value: String,
}

impl VariableValue {
    pub fn new(type_: String, value: String) -> Self {
        VariableValue { type_, value }
    }

    pub fn type_(&self) -> &str {
        &self.type_
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug)]
pub struct LLDBProcess {
    debugger_cmd: Option<String>,
    run_cmd: Option<Vec<String>>,
    lldb_process: Option<Child>,
    lldb_stdin_tx: Option<Sender<Bytes>>,
    analyser: Arc<Mutex<Analyser>>,
}

impl LLDBProcess {
    /// Create a new LLDBProcess
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        LLDBProcess {
            debugger_cmd: Some(debugger_cmd),
            run_cmd: Some(run_cmd),
            lldb_process: None,
            lldb_stdin_tx: None,
            analyser: Arc::new(Mutex::new(Analyser::new())),
        }
    }

    /// Setup LLDB
    ///
    /// Includes spawning the LLDB process and all the relevant stdio handlers. In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read stdout and stderr;
    /// - Sets up a thread to read stdin and forward it onto LLDB stdin;
    /// - Checks that LLDB and the program to be ran both exist, otherwise panics.
    pub fn setup(&mut self) {
        let mut lldb_process = check_and_spawn_process(
            vec![self.debugger_cmd.take().unwrap()],
            self.run_cmd.take().unwrap(),
        );

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
        //let stdin_tx = setup_stdin(
        //    lldb_process
        //        .stdin()
        //        .take()
        //        .expect("LLDB process did not have a handle to stdin"),
        //    false,
        //);

        //self.lldb_stdin_tx = Some(stdin_tx);
        self.lldb_process = Some(lldb_process);
    }

    pub fn teardown(&mut self) {
        self.lldb_process = None;
    }

    /// Send a message to write to stdin
    pub fn write_stdin(&mut self, bytes: Bytes) {
        //let tx = self.lldb_stdin_tx.clone();
        //tokio::spawn(
        //    tx.clone()
        //        .unwrap()
        //        .send(bytes)
        //        .map(move |_| {})
        //        .map_err(|e| eprintln!("Error sending to LLDB: {}", e)),
        //);
    }

    pub fn add_listener(&mut self, kind: Listener, sender: Sender<Event>) {
        self.analyser.lock().unwrap().add_listener(kind, sender);
    }

    pub fn is_process_running(&self) -> bool {
        self.analyser.lock().unwrap().is_process_running()
    }

    /// Perform setup of reading LLDB stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let analyser = self.analyser.clone();
        tokio::spawn(
            read_output(BufReader::new(stdout))
                .for_each(move |text| {
                    print!("{}", text);
                    analyser.lock().unwrap().analyse_stdout(&text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading LLDB stdout: {}", e)),
        );
    }

    /// Perform setup of reading LLDB stderr, analysing it and writing it back to stdout.
    fn setup_stderr(&mut self, stderr: ChildStderr) {
        let analyser = self.analyser.clone();
        tokio::spawn(
            read_output(BufReader::new(stderr))
                .for_each(move |text| {
                    eprint!("{}", text);
                    analyser.lock().unwrap().analyse_stderr(&text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading LLDB stderr: {}", e)),
        );
    }
}

#[derive(Debug)]
pub struct Analyser {
    stdout: String,
    stderr: String,
    process_pid: Option<u64>,
    listeners: HashMap<Listener, Sender<Event>>,
}

impl Analyser {
    pub fn new() -> Self {
        Analyser {
            stdout: "".to_string(),
            stderr: "".to_string(),
            process_pid: None,
            listeners: HashMap::new(),
        }
    }

    pub fn add_listener(&mut self, kind: Listener, sender: Sender<Event>) {
        self.listeners.insert(kind, sender);
    }

    pub fn analyse_stdout(&mut self, s: &str) {
        self.stdout.push_str(s);

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
            static ref RE_PRINTED_VARIABLE: Regex =
                Regex::new("^\\((.*)\\) ([\\S+]*) = .*$").unwrap();
            static ref RE_PROCESS_NOT_RUNNING: Regex =
                Regex::new("error: invalid process$").unwrap();
        }

        let s = self.stdout.clone();

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

            for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
                let variable_type = cap[1].to_string();
                let variable = cap[2].to_string();
                self.printed_variable(variable, variable_type, &s);
            }

            for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
                self.process_not_running();
            }
        }

        self.clear_analyser();
    }

    pub fn analyse_stderr(&mut self, s: &str) {
        self.stderr.push_str(s);

        lazy_static! {
            static ref RE_VARIABLE_NOT_FOUND: Regex =
                Regex::new("error: no variable named '([^']*)' found in this frame$").unwrap();
        }

        let s = self.stderr.clone();

        for line in s.split("\n") {
            for cap in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
                let variable = cap[1].to_string();
                self.variable_not_found(variable);
            }
        }

        self.clear_analyser();
    }

    fn clear_analyser(&mut self) {
        self.stdout = "".to_string();
        self.stderr = "".to_string();
    }

    pub fn is_process_running(&self) -> bool {
        match self.process_pid {
            Some(_) => true,
            None => false,
        }
    }

    fn lldb_started(&mut self) {
        match self.listeners.remove(&Listener::LLDBLaunched) {
            Some(listener) => {
                listener.send(Event::LLDBLaunched).wait().unwrap();
            }
            None => {}
        }
    }

    fn process_started(&mut self, pid: u64) {
        self.process_pid = Some(pid);
        match self.listeners.remove(&Listener::ProcessLaunched) {
            Some(listener) => {
                listener.send(Event::ProcessLaunched(pid)).wait().unwrap();
            }
            None => {}
        }
    }

    fn process_exited(&mut self, pid: u64, exit_code: i64) {
        self.process_pid = None;
        signal_exited(pid, exit_code);
        match self.listeners.remove(&Listener::ProcessExited) {
            Some(listener) => {
                listener
                    .send(Event::ProcessExited(pid, exit_code))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn found_breakpoint(&mut self, file: String, line: u64) {
        breakpoint_set(&file, line);
        let file_location = FileLocation::new(file, line);
        match self.listeners.remove(&Listener::Breakpoint) {
            Some(listener) => {
                listener
                    .send(Event::BreakpointSet(file_location))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn found_multiple_breakpoints(&mut self) {
        match self.listeners.remove(&Listener::Breakpoint) {
            Some(listener) => {
                listener.send(Event::BreakpointMultiple).wait().unwrap();
            }
            None => {}
        }
    }

    fn found_pending_breakpoint(&mut self) {
        match self.listeners.remove(&Listener::Breakpoint) {
            Some(listener) => {
                listener.send(Event::BreakpointPending).wait().unwrap();
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

    fn printed_variable(&mut self, variable: String, variable_type: String, data: &str) {
        let mut start = 1;

        while &data[start..start + 1] != ")" {
            start += 1;
        }
        while &data[start..start + 1] != "=" {
            start += 1;
        }
        start += 2;

        // TODO: Need a better way of doing this to strip of the last \n,
        // it's possible one day we'll screw the UTF-8 pooch here.
        let value = data[start..data.len() - 1].to_string();

        match self.listeners.remove(&Listener::PrintVariable) {
            Some(listener) => {
                let variable = Variable::new(variable);
                let value = VariableValue::new(variable_type, value);
                listener
                    .send(Event::PrintVariable(variable, value))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }

    fn process_not_running(&self) {
        log_msg(LogLevel::WARN, "program not running");
    }

    fn variable_not_found(&mut self, variable: String) {
        match self.listeners.remove(&Listener::PrintVariable) {
            Some(listener) => {
                let variable = Variable::new(variable);
                listener
                    .send(Event::VariableNotFound(variable))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }
}
