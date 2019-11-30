//! Python process handler
//!
//! This module performs the basic setup of and interfacing with Python through
//! the pdb module. It will analyse the output of the text and work out what is
//! happening then.

use std::collections::HashMap;
use std::path::Path;
#[cfg(not(test))]
use std::process::exit;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use padre_core::server::{FileLocation, Variable};
use padre_core::notifier::{breakpoint_set, jump_to_position, log_msg, signal_exited, LogLevel};
#[cfg(not(test))]
use padre_core::util::{file_exists, get_file_full_path};
use padre_core::util::{read_output, setup_stdin};

use bytes::Bytes;
use futures::prelude::*;
use regex::Regex;
use tokio::io::BufReader;
use tokio::process::{Command, Child, ChildStderr, ChildStdout};
use tokio::sync::mpsc::Sender;

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

/// Main handler for spawning the Python process
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
        let debugger_cmd = self.debugger_cmd.take().unwrap();
        let run_cmd = self.run_cmd.take().unwrap();

        let args = get_python_args(&debugger_cmd[..], run_cmd.iter().map(|x| &x[..]).collect());

        let mut process = Command::new(&debugger_cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn debugger");

        log_msg(LogLevel::INFO, &format!("Process launched with pid: {}", process.id()));

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
        tokio::spawn(async move {
            tx.clone()
                .unwrap()
                .send(bytes)
                .map(move |_| {})
                .await
        });
    }

    /// Perform setup of reading Python stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let analyser = self.analyser.clone();
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                analyser.lock().unwrap().analyse_stdout(&text);
            }
        });
    }

    /// Perform setup of reading Python stderr, analysing it and writing it back to stdout.
    fn setup_stderr(&mut self, stderr: ChildStderr) {
        tokio::spawn(async {
            let mut reader = read_output(BufReader::new(stderr));
            while let Some(Ok(text)) = reader.next().await {
                eprint!("{}", text);
            }
        });
    }
}

/// Work out the arguments to send to python based on the python command given and the
/// run command specified
fn get_python_args<'a>(debugger_cmd: &str, run_cmd: Vec<&'a str>) -> Vec<&'a str> {
    let mut python_args = vec![];
    let mut script_args = vec![];

    // Now check the debugger and program to debug exist, if not error
    #[cfg(not(test))]
    {
        // Try getting the full path if the debugger doesn't exist
        if !file_exists(&debugger_cmd) {
            let debugger_cmd = get_file_full_path(&debugger_cmd);

            if !file_exists(&debugger_cmd) {
                let msg = format!("Can't spawn debugger as {} does not exist", debugger_cmd);
                log_msg(LogLevel::CRITICAL, &msg);
                println!("{}", msg);

                exit(1);
            }
        }
    }

    python_args.push("-m");
    python_args.push("pdb");

    // If we have the command `python -m mymod` say and `python` is specified
    // as the debugger then we have then we don't want to run
    // `python -m pdb -- -m mymod`
    // On the other hand if we specified `./script.py -a test` we want that to
    // run
    // `python -m pdb -- ./script.py -a test`
    // so we keep track of whether they're likely to be a python arg or a script
    // arg here.
    //
    // tl;dr We assume all args are script args if the 0th element doesn't
    // match the debugger, if it does we wait until we see `--` and then we
    // assume script args.
    let mut is_script_arg = true;

    for (i, arg) in run_cmd.iter().enumerate() {
        // Skip the python part if specified as we add that from the -d option
        if i == 0 {
            let debugger_cmd_path = Path::new(debugger_cmd);

            let debugger_cmd = match debugger_cmd_path.file_name() {
                Some(s) => s.to_str().unwrap(),
                None => debugger_cmd,
            };

            if debugger_cmd == *arg {
                is_script_arg = false;
                continue;
            } else {
                is_script_arg = true;
            }
        }

        if *arg == "--" {
            is_script_arg = true;
            continue;
        }

        if is_script_arg {
            script_args.push(&arg[..]);
        } else {
            python_args.push(arg);
        }
    }

    if script_args.len() > 0 {
        python_args.push("--");
        python_args.append(&mut script_args);
    }

    python_args
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
            static ref RE_RETURNING: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)->(.*)$").unwrap();
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

            for cap in RE_RETURNING.captures_iter(line) {
                let file = cap[1].to_string();
                let line = cap[2].parse::<u64>().unwrap();
                let return_value = cap[3].to_string();
                jump_to_position(&file, line);
                log_msg(LogLevel::INFO, &format!("Returning value {}", return_value));
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
            Some(mut listener) => {
                tokio::spawn(async move {
                    listener.send(Event::Launched).await.unwrap();
                });
            }
            None => {}
        }
    }

    fn found_breakpoint(&mut self, file: String, line: u64) {
        breakpoint_set(&file, line);
        let file_location = FileLocation::new(file, line);
        match self.listeners.remove(&Listener::Breakpoint) {
            Some(mut listener) => {
                tokio::spawn(async move {
                    listener
                        .send(Event::BreakpointSet(file_location))
                        .await
                        .unwrap();
                });
            }
            None => {}
        }
    }

    fn print_variable(&mut self, variable: Variable, data: &str) {
        let len = data.len();
        if len < 2 {
            return;
        }

        let to = data.len() - 2;
        match self.listeners.remove(&Listener::PrintVariable) {
            Some(mut listener) => {
                let event = Event::PrintVariable(variable, data[0..to].to_string());
                tokio::spawn(async move {
                    listener
                        .send(event)
                        .await
                        .unwrap();
                });
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_get_args_basic_command() {
        let args = super::get_python_args("/usr/bin/python3", vec!["test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "--", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_recognises_matching_python_command() {
        let args = super::get_python_args("/usr/bin/python3", vec!["python3", "test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_leaves_non_matching_python_command() {
        let args = super::get_python_args("/usr/bin/python3.7", vec!["python3", "test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "--", "python3", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_module_running() {
        let args = super::get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-m", "abc", "--", "arg1"],
        );
        assert_eq!(args, vec!["-m", "pdb", "-m", "abc", "--", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_command_arguments() {
        let args = super::get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-c", "print('Hello, World!')"],
        );
        assert_eq!(args, vec!["-m", "pdb", "-c", "print('Hello, World!')"]);
    }
}
