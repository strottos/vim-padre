//! Python process handler
//!
//! This module performs the basic setup of and interfacing with Python through
//! the pdb module. It will analyse the output of the text and work out what is
//! happening then.

use std::env;
use std::io::{self, Write};
use std::path::Path;
#[cfg(not(test))]
use std::process::exit;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use padre_core::debugger::{FileLocation, Variable};
use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
#[cfg(not(test))]
use padre_core::util::{file_exists, get_file_full_path};
use padre_core::util::{read_output, setup_stdin};

use bytes::Bytes;
use futures::prelude::*;
use regex::Regex;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc::Sender;

/// Messages that can be sent to PDB for processing
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    Launching,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
    Custom,
}

/// Current status of PDB
#[derive(Debug, Clone, PartialEq)]
pub enum PDBStatus {
    None,
    Listening,
    Processing(Message),
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

        let mut pty_wrapper = env::current_exe().unwrap();
        pty_wrapper.pop();
        pty_wrapper.pop();
        pty_wrapper.pop();
        pty_wrapper.push("ptywrapper.py");

        let mut process = Command::new(pty_wrapper)
            .arg(&debugger_cmd)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn debugger");

        log_msg(
            LogLevel::INFO,
            &format!("Process launched with pid: {}", process.id()),
        );

        self.setup_stdout(
            process
                .stdout
                .take()
                .expect("Python process did not have a handle to stdout"),
        );
        let stdin_tx = setup_stdin(
            process
                .stdin
                .take()
                .expect("Python process did not have a handle to stdin"),
            false,
        );

        self.analyser.lock().unwrap().set_pid(process.id() as u64);

        self.stdin_tx = Some(stdin_tx);
        self.process = Some(process);
    }

    /// Adds a Sender object that gets awoken when we are listening.
    ///
    /// Should only add a sender when we're about to go into or currently in the
    /// processing status otherwise this will never wake up.
    pub fn add_awakener(&self, sender: Sender<bool>) {
        self.analyser.lock().unwrap().add_awakener(sender);
    }

    /// Drop the awakener
    pub fn drop_awakener(&mut self) {
        self.analyser.lock().unwrap().drop_awakener();
    }

    /// Check the current status, either not running (None), running something
    /// (Processing) or listening for a message on PDB (Listening).
    pub fn get_status(&self) -> PDBStatus {
        self.analyser.lock().unwrap().get_status()
    }

    /// Send a message to write to stdin
    pub fn send_msg(&mut self, message: Message) {
        let tx = self.stdin_tx.clone();
        let analyser = self.analyser.clone();

        tokio::spawn(async move {
            let msg = match &message {
                Message::Breakpoint(fl) => {
                    Bytes::from(format!("break {}:{}\n", fl.name(), fl.line_num()))
                }
                Message::Unbreakpoint(fl) => {
                    Bytes::from(format!("clear {}:{}\n", fl.name(), fl.line_num()))
                }
                Message::StepIn => Bytes::from("step\n"),
                Message::StepOver => Bytes::from("next\n"),
                Message::Continue => Bytes::from("continue\n"),
                Message::Launching => unreachable!(),
                Message::PrintVariable(v) => Bytes::from(format!("print({})\n", v.name())),
                Message::Custom => todo!(),
            };

            analyser.lock().unwrap().analyse_message(message);

            tx.clone().unwrap().send(msg).map(move |_| {}).await
        });
    }

    /// Perform setup of reading Python stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let analyser = self.analyser.clone();
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                io::stdout().flush().unwrap();
                analyser.lock().unwrap().analyse_stdout(&text);
            }
        });
    }
}

#[derive(Debug)]
pub struct Analyser {
    status: PDBStatus,
    pid: Option<u64>,
    awakener: Option<Sender<bool>>,
    // For keeping track of the variable that we were told to print
    variable_value: String,
}

impl Analyser {
    pub fn new() -> Self {
        Analyser {
            status: PDBStatus::None,
            pid: None,
            awakener: None,
            variable_value: "".to_string(),
        }
    }

    pub fn get_status(&mut self) -> PDBStatus {
        self.status.clone()
    }

    /// Add the awakener to send a message to when we awaken
    pub fn add_awakener(&mut self, sender: Sender<bool>) {
        self.awakener = Some(sender);
    }

    /// Drop the awakener
    pub fn drop_awakener(&mut self) {
        self.awakener = None;
    }

    pub fn analyse_stdout(&mut self, s: &str) {
        lazy_static! {
            static ref RE_RETURNING: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)->(.*)$").unwrap();
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("^The program finished and will be restarted$").unwrap();
            static ref RE_PROCESS_EXITED_WITH_CODE: Regex =
                Regex::new("^The program exited via sys.exit\\(\\)\\. Exit status: (-?\\d*)$")
                    .unwrap();
        }

        match self.get_status() {
            PDBStatus::Processing(msg) => {
                match msg {
                    Message::PrintVariable(var) => {
                        let mut from = 0;
                        let mut to = s.len();

                        let print_cmd_size = 7 + var.name().len();
                        if to >= print_cmd_size + 2
                            && &s[0..print_cmd_size] == &format!("print({})", var.name())
                        {
                            // 2 extra for \r\n
                            from += print_cmd_size + 2;
                        }

                        println!("s: {}", s);
                        println!("from: {}", from);
                        println!("to: {}", to);

                        let pdb_length = "(Pdb) ".len();
                        if to >= pdb_length && &s[to - pdb_length..to] == "(Pdb) " {
                            to -= pdb_length;
                        }

                        self.variable_value += &s[from..to];
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        for line in s.split("\r\n") {
            if line == "(Pdb) " {
                match self.get_status() {
                    PDBStatus::Processing(msg) => match msg {
                        Message::PrintVariable(var) => {
                            let mut to = self.variable_value.len();
                            if to >= 2 && &self.variable_value[to - 2..to] == "\r\n" {
                                to -= 2;
                            }
                            let msg = format!("{}={}", var.name(), &self.variable_value[0..to]);
                            log_msg(LogLevel::INFO, &msg);
                            self.variable_value = "".to_string();
                        }
                        _ => {}
                    },
                    _ => {}
                }

                self.set_listening();
                return;
            }

            match self.get_status() {
                PDBStatus::None => {
                    self.check_location(line);
                    self.status = PDBStatus::Processing(Message::Launching)
                }
                PDBStatus::Listening => self.status = PDBStatus::Processing(Message::Custom),
                PDBStatus::Processing(msg) => match msg {
                    Message::Breakpoint(_) => {
                        self.check_breakpoint(line);
                    }
                    Message::StepIn | Message::StepOver | Message::Continue => {
                        self.check_location(line);
                    }
                    Message::Custom => {
                        self.check_breakpoint(line);
                        self.check_location(line);
                    }
                    _ => {}
                },
            };

            for cap in RE_RETURNING.captures_iter(line) {
                let file = cap[1].to_string();
                let line = cap[2].parse::<u64>().unwrap();
                let return_value = cap[3].to_string();
                jump_to_position(&file, line);
                self.set_listening();
                log_msg(LogLevel::INFO, &format!("Returning value {}", return_value));
            }

            for _ in RE_PROCESS_EXITED.captures_iter(line) {
                signal_exited(self.pid.unwrap(), 0);
            }

            for cap in RE_PROCESS_EXITED_WITH_CODE.captures_iter(line) {
                let exit_code = cap[1].parse::<i64>().unwrap();
                signal_exited(self.pid.unwrap(), exit_code);
            }
        }
    }

    pub fn analyse_message(&mut self, msg: Message) {
        self.status = PDBStatus::Processing(msg);
    }

    pub fn set_pid(&mut self, pid: u64) {
        self.pid = Some(pid);
    }

    fn set_listening(&mut self) {
        self.status = PDBStatus::Listening;
        let awakener = self.awakener.take();
        match awakener {
            Some(mut x) => {
                tokio::spawn(async move {
                    x.send(true).await.unwrap();
                });
            }
            None => {}
        };
    }

    fn check_breakpoint(&self, line: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex =
                Regex::new("^Breakpoint (\\d*) at (.*):(\\d*)$").unwrap();
        }

        for cap in RE_BREAKPOINT.captures_iter(line) {
            let file = cap[2].to_string();
            let line = cap[3].parse::<u64>().unwrap();
            log_msg(
                LogLevel::INFO,
                &format!("Breakpoint set at file {} and line number {}", file, line),
            );
        }
    }

    fn check_location(&self, line: &str) {
        lazy_static! {
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)$").unwrap();
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            let file = cap[1].to_string();
            let line = cap[2].parse::<u64>().unwrap();
            jump_to_position(&file, line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_get_args_basic_command() {
        let args = get_python_args("/usr/bin/python3", vec!["test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "--", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_recognises_matching_python_command() {
        let args = get_python_args("/usr/bin/python3", vec!["python3", "test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_leaves_non_matching_python_command() {
        let args = get_python_args("/usr/bin/python3.7", vec!["python3", "test.py", "arg1"]);
        assert_eq!(args, vec!["-m", "pdb", "--", "python3", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_module_running() {
        let args = get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-m", "abc", "--", "arg1"],
        );
        assert_eq!(args, vec!["-m", "pdb", "-m", "abc", "--", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_command_arguments() {
        let args = get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-c", "print('Hello, World!')"],
        );
        assert_eq!(args, vec!["-m", "pdb", "-c", "print('Hello, World!')"]);
    }

    #[test]
    fn analyser_startup() {
        let mut analyser = Analyser::new();
        assert_eq!(analyser.get_status(), PDBStatus::None);
        analyser.analyse_stdout("> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n");
        assert_eq!(
            analyser.get_status(),
            PDBStatus::Processing(Message::Launching)
        );
        analyser.analyse_stdout("(Pdb) ");
        assert_eq!(analyser.get_status(), PDBStatus::Listening);
    }

    #[test]
    fn analyser_custom_syntax_error() {
        let mut analyser = Analyser::new();
        analyser.analyse_stdout("> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ");
        analyser.analyse_stdout("do something nonsensical\r\n");
        assert_eq!(
            analyser.get_status(),
            PDBStatus::Processing(Message::Custom)
        );
        analyser.analyse_stdout("*** SyntaxError: invalid syntax\r\n(Pdb) ");
        assert_eq!(analyser.get_status(), PDBStatus::Listening);
    }

    #[test]
    fn analyser_padre_message() {
        let mut analyser = Analyser::new();
        analyser.analyse_stdout("> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ");
        let msg = Message::Breakpoint(FileLocation::new("test.py".to_string(), 2));
        analyser.analyse_message(msg.clone());
        assert_eq!(analyser.get_status(), PDBStatus::Processing(msg));
        analyser.analyse_stdout("Breakpoint 1 at test.py:2\r\n(Pdb) ");
        assert_eq!(analyser.get_status(), PDBStatus::Listening);
    }

    #[test]
    fn analyser_print_message() {
        let mut analyser = Analyser::new();
        analyser.analyse_stdout("> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ");
        let msg = Message::PrintVariable(Variable::new("abc".to_string()));
        analyser.analyse_message(msg.clone());
        analyser.analyse_stdout("print(abc)\r\n");
        analyser.analyse_stdout("123\r\n");
        assert_eq!(analyser.variable_value, "123\r\n");
        analyser.analyse_stdout("(Pdb) ");
        analyser.analyse_message(msg.clone());
        analyser.analyse_stdout("print(abc)\r\n\"abcd1234\"\r\n");
        assert_eq!(analyser.variable_value, "\"abcd1234\"\r\n");
        analyser.analyse_stdout("(Pdb) ");
    }
}
