//! Python process handler
//!
//! This module performs the basic setup of and interfacing with Python through
//! the pdb module. It will analyse the output of the text and work out what is
//! happening then.

use std::collections::HashMap;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

use crate::debugger::{FileLocation, Variable};
use crate::notifier::{breakpoint_set, jump_to_position, signal_exited};
use crate::util::{check_and_spawn_process, read_output, setup_stdin};

use bytes::Bytes;
use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::Sender;
use tokio_process::{Child, ChildStderr, ChildStdout};

#[derive(Debug, Clone, PartialEq)]
pub enum PDBStatus {
    None,
    Running,
    Printing(Variable),
}

/// You can register to listen for one of the following events:
/// - Breakpoint: A breakpoint event has happened
/// - PrintVariable: A variable printing event
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Listener {
    Launch,
    Breakpoint,
    PrintVariable,
}

/// A Python event is something that can be registered for being listened to and can be triggered
/// when these events occur such that the listener is informed of them and passed some details
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Event {
    Launched,
    BreakpointSet(FileLocation),
    PrintVariable(Variable, String),
}

#[derive(Debug)]
pub struct Process {
    debugger_cmd: Option<String>,
    run_cmd: Option<Vec<String>>,
    process: Option<Child>,
    stdin_tx: Option<Sender<Bytes>>,
    analyser: Arc<Mutex<Analyser>>,
}

impl Process {
    /// Create a new Process
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        Process {
            debugger_cmd: Some(debugger_cmd),
            run_cmd: Some(run_cmd),
            process: None,
            stdin_tx: None,
            analyser: Arc::new(Mutex::new(Analyser::new())),
        }
    }

    /// Run Python program including loading the pdb module for debugging
    ///
    /// Includes spawning the Python process and setting up all the relevant stdio handlers.
    /// In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read stdout and stderr;
    /// - Sets up a thread to read stdin and forward it onto Python interpreter;
    /// - Checks that Python and the program to be ran both exist, otherwise panics.
    pub fn run(&mut self) {
        let mut process = check_and_spawn_process(
            vec![
                self.debugger_cmd.take().unwrap(),
                "-m".to_string(),
                "pdb".to_string(),
            ],
            self.run_cmd.take().unwrap(),
        );

        self.setup_stdout(
            process
                .stdout()
                .take()
                .expect("Python process did not have a handle to stdout"),
        );
        self.setup_stderr(
            process
                .stderr()
                .take()
                .expect("Python process did not have a handle to stderr"),
        );
        let stdin_tx = setup_stdin(
            process
                .stdin()
                .take()
                .expect("Python process did not have a handle to stdin"),
            true,
        );

        self.analyser.lock().unwrap().set_pid(process.id() as u64);

        self.stdin_tx = Some(stdin_tx);
        self.process = Some(process);
    }

    pub fn add_listener(&self, kind: Listener, sender: Sender<Event>) {
        self.analyser.lock().unwrap().add_listener(kind, sender);
    }

    pub fn get_pid(&self) -> u64 {
        self.process.as_ref().unwrap().id() as u64
    }

    pub fn get_status(&self) -> PDBStatus {
        self.analyser.lock().unwrap().get_status()
    }

    pub fn set_status(&self, status: PDBStatus) {
        self.analyser.lock().unwrap().status = status;
    }

    /// Send a message to write to stdin
    pub fn write_stdin(&mut self, bytes: Bytes) {
        let tx = self.stdin_tx.clone();
        tokio::spawn(
            tx.clone()
                .unwrap()
                .send(bytes)
                .map(move |_| {})
                .map_err(|e| eprintln!("Error sending to Python: {}", e)),
        );
    }

    /// Perform setup of reading Python stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let analyser = self.analyser.clone();
        tokio::spawn(
            read_output(BufReader::new(stdout))
                .for_each(move |text| {
                    print!("{}", text);
                    analyser.lock().unwrap().analyse_stdout(&text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading Python stdout: {}", e)),
        );
    }

    /// Perform setup of reading Python stderr, analysing it and writing it back to stdout.
    fn setup_stderr(&mut self, stderr: ChildStderr) {
        tokio::spawn(
            read_output(BufReader::new(stderr))
                .for_each(move |text| {
                    eprint!("{}", text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading Python stderr: {}", e)),
        );
    }
}

#[derive(Debug)]
pub struct Analyser {
    status: PDBStatus,
    pid: Option<u64>,
    listeners: HashMap<Listener, Sender<Event>>,
}

impl Analyser {
    pub fn new() -> Self {
        Analyser {
            status: PDBStatus::None,
            pid: None,
            listeners: HashMap::new(),
        }
    }

    pub fn get_status(&mut self) -> PDBStatus {
        self.status.clone()
    }

    pub fn analyse_stdout(&mut self, s: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex =
                Regex::new("^Breakpoint (\\d*) at (.*):(\\d*)$").unwrap();
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)$").unwrap();
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("^The program finished and will be restarted$").unwrap();
            static ref RE_PROCESS_EXITED_WITH_CODE: Regex =
                Regex::new("^The program exited via sys.exit\\(\\)\\. Exit status: (-?\\d*)$")
                    .unwrap();
        }

        for line in s.split("\n") {
            if line.contains("(Pdb) ") {
                match self.status {
                    PDBStatus::None => {
                        self.python_launched();
                    }
                    _ => {}
                };
            }

            for cap in RE_BREAKPOINT.captures_iter(line) {
                let file = cap[2].to_string();
                let line = cap[3].parse::<u64>().unwrap();
                self.found_breakpoint(file, line);
            }

            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                let file = cap[1].to_string();
                let line = cap[2].parse::<u64>().unwrap();
                jump_to_position(&file, line);
            }

            for _ in RE_PROCESS_EXITED.captures_iter(line) {
                signal_exited(self.pid.unwrap(), 0);
            }

            for cap in RE_PROCESS_EXITED_WITH_CODE.captures_iter(line) {
                let exit_code = cap[1].parse::<i64>().unwrap();
                signal_exited(self.pid.unwrap(), exit_code);
            }
        }

        match self.status.clone() {
            PDBStatus::Printing(var) => {
                self.print_variable(var, s);
            }
            _ => {}
        }
    }

    pub fn add_listener(&mut self, kind: Listener, sender: Sender<Event>) {
        self.listeners.insert(kind, sender);
    }

    pub fn set_pid(&mut self, pid: u64) {
        self.pid = Some(pid);
    }

    fn python_launched(&mut self) {
        self.status = PDBStatus::Running;
        match self.listeners.remove(&Listener::Launch) {
            Some(listener) => {
                listener.send(Event::Launched).wait().unwrap();
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

    fn print_variable(&mut self, variable: Variable, data: &str) {
        let to = data.len() - 2;
        match self.listeners.remove(&Listener::PrintVariable) {
            Some(listener) => {
                listener
                    .send(Event::PrintVariable(variable, data[0..to].to_string()))
                    .wait()
                    .unwrap();
            }
            None => {}
        }
    }
}
