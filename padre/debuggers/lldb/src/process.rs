//! lldb process handler
//!
//! This module performs the basic setup of and interfacing with LLDB. It will
//! analyse the output of the text and work out what is happening then.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
use padre_core::server::{FileLocation, Variable};
use padre_core::util::{check_and_spawn_process, read_output};

use bytes::Bytes;
use futures::prelude::*;
use regex::Regex;
use tokio::io::{stdin, BufReader};
use tokio::prelude::*;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to LLDB for processing
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    ProcessLaunching,
    Breakpoint(FileLocation),
    UnknownBreakpoint,
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
}

/// Current status of LLDB
#[derive(Debug, Clone, PartialEq)]
pub enum LLDBStatus {
    None,
    Listening,
    Processing(Message),
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

/// Main handler for spawning the LLDB process
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
        self.setup_stdin(
            lldb_process
                .stdin()
                .take()
                .expect("Python process did not have a handle to stdin"),
            true,
        );

        self.lldb_process = Some(lldb_process);
    }

    pub fn teardown(&mut self) {
        self.lldb_process = None;
    }

    /// Send a message to write to stdin
    pub fn write_stdin(&mut self, bytes: Bytes) {
        let lldb_stdin_tx = self.lldb_stdin_tx.clone();
        tokio::spawn(async move {
            lldb_stdin_tx
                .clone()
                .unwrap()
                .send(bytes)
                .map(move |_| {})
                .await;
        });
    }

    pub fn send_msg(&mut self, message: Message) {
        let msg_bytes = match message.clone() {
            Message::ProcessLaunching => Bytes::from("process launch\n"),
            Message::Breakpoint(fl) => Bytes::from(format!(
                "breakpoint set --file {} --line {}\n",
                fl.name(),
                fl.line_num()
            )),
            Message::UnknownBreakpoint => unreachable!(),
            Message::StepIn => Bytes::from("thread step-in\n"),
            Message::StepOver => Bytes::from("thread step-over\n"),
            Message::Continue => Bytes::from("thread continue\n"),
            Message::PrintVariable(v) => Bytes::from(format!("frame variable {}\n", v.name())),
        };

        self.analyser.lock().unwrap().status = LLDBStatus::Processing(message);
        self.write_stdin(msg_bytes);
    }

    /// Adds a Sender object that gets awoken when we are listening.
    ///
    /// Should only add a sender when we're about to go into or currently in the
    /// processing status otherwise this will never wake up.
    pub fn add_awakener(&mut self, sender: Sender<bool>) {
        self.analyser.lock().unwrap().add_awakener(sender);
    }

    /// Drop the awakener
    pub fn drop_awakener(&mut self) {
        self.analyser.lock().unwrap().drop_awakener();
    }

    pub fn is_process_running(&self) -> bool {
        self.analyser.lock().unwrap().is_process_running()
    }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(&mut self, mut child_stdin: ChildStdin, output_stdin: bool) {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(32);
        let mut tx = stdin_tx.clone();

        let analyser = self.analyser.clone();

        tokio::spawn(async move {
            let tokio_stdin = stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            while let Some(line) = reader.next().await {
                let buf = line.unwrap().freeze();

                let mut start = 0;

                if buf.len() >= 7 && buf[0..7] == b"(lldb) "[..] {
                    println!("DETECTED `(lldb) `, skipping");
                    start = 7;
                }

                if buf.len() >= start + 2 {
                    println!("stuff {:?}", &buf[start..start + 2]);
                }
                if buf.len() >= start + 3 {
                    println!("stuff {:?}", &buf[start..start + 3]);
                }
                if buf.len() >= start + 11 {
                    println!("stuff {:?}", &buf[start..start + 11]);
                }

                if (buf.len() >= start + 2 && buf[start..start + 2] == b"b "[..])
                    || (buf.len() >= start + 3 && buf[start..start + 3] == b"br "[..])
                    || (buf.len() >= start + 11 && buf[start..start + 11] == b"breakpoint "[..])
                {
                    println!("UNKNOWN BREAKPOINT");
                    analyser.lock().unwrap().status =
                        LLDBStatus::Processing(Message::UnknownBreakpoint);
                }

                tx.send(buf).await.unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(text) = stdin_rx.next().await {
                if output_stdin {
                    io::stdout().write_all(&text).unwrap();
                }
                match child_stdin.write(&text).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Writing stdin err e: {}", e);
                    }
                };
            }
        });

        self.lldb_stdin_tx = Some(stdin_tx);
    }

    /// Perform setup of reading LLDB stdout, analysing it and writing it back to stdout.
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

    /// Perform setup of reading LLDB stderr, analysing it and writing it back to stdout.
    fn setup_stderr(&mut self, stderr: ChildStderr) {
        let analyser = self.analyser.clone();
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stderr));
            while let Some(Ok(text)) = reader.next().await {
                eprint!("{}", text);
                analyser.lock().unwrap().analyse_stderr(&text);
            }
        });
    }
}

#[derive(Debug)]
pub struct Analyser {
    status: LLDBStatus,
    stdout: String,
    stderr: String,
    process_pid: Option<u64>,
    awakener: Option<Sender<bool>>,
}

impl Analyser {
    pub fn new() -> Self {
        Analyser {
            status: LLDBStatus::None,
            stdout: "".to_string(),
            stderr: "".to_string(),
            process_pid: None,
            awakener: None,
        }
    }

    /// Add the awakener to send a message to when we awaken
    pub fn add_awakener(&mut self, sender: Sender<bool>) {
        self.awakener = Some(sender);
    }

    /// Drop the awakener
    pub fn drop_awakener(&mut self) {
        self.awakener = None;
    }

    pub fn get_status(&self) -> &LLDBStatus {
        &self.status
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
            static ref RE_SETTINGS: Regex = Regex::new("settings ").unwrap();
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
                self.set_listening();
            }

            if !found_breakpoint {
                for cap in RE_BREAKPOINT_2.captures_iter(line) {
                    found_breakpoint = true;
                    let file = cap[2].to_string();
                    let line = cap[3].parse::<u64>().unwrap();
                    self.found_breakpoint(file, line);
                    self.set_listening();
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_MULTIPLE.captures_iter(line) {
                    found_breakpoint = true;
                    self.found_multiple_breakpoints();
                    self.set_listening();
                }
            }

            if !found_breakpoint {
                for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
                    self.found_pending_breakpoint();
                    self.set_listening();
                }
            }

            for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
                let mut found = false;
                for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                    found = true;
                    let file = cap[1].to_string();
                    let line = cap[2].parse::<u64>().unwrap();
                    self.jump_to_position(file, line);
                    self.set_listening();
                }

                if !found {
                    self.jump_to_unknown_position();
                    self.set_listening();
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

            for _ in RE_SETTINGS.captures_iter(line) {
                self.set_listening();
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

    fn set_listening(&mut self) {
        self.status = LLDBStatus::Listening;
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
        self.set_listening();
    }

    fn process_started(&mut self, pid: u64) {
        self.set_listening();
    }

    fn process_exited(&mut self, pid: u64, exit_code: i64) {
        self.process_pid = None;
        signal_exited(pid, exit_code);
        self.set_listening();
    }

    fn found_breakpoint(&mut self, file: String, line: u64) {
        match &self.status {
            LLDBStatus::Processing(msg) => {
                match msg {
                    Message::Breakpoint(_) | Message::UnknownBreakpoint => {
                        log_msg(
                            LogLevel::INFO,
                            &format!("Breakpoint set at file {} and line number {}", file, line),
                        );
                    }
                    _ => {}
                };
            }
            _ => {}
        };
        //breakpoint_set(&file, line);
        //let file_location = FileLocation::new(file, line);
        //match self.listeners.remove(&Listener::Breakpoint) {
        //    Some(listener) => {
        //        listener
        //            .send(Event::BreakpointSet(file_location))
        //            .wait()
        //            .unwrap();
        //    }
        //    None => {}
        //}
    }

    fn found_multiple_breakpoints(&mut self) {
        //match self.listeners.remove(&Listener::Breakpoint) {
        //    Some(listener) => {
        //        listener.send(Event::BreakpointMultiple).wait().unwrap();
        //    }
        //    None => {}
        //}
    }

    fn found_pending_breakpoint(&mut self) {
        //match self.listeners.remove(&Listener::Breakpoint) {
        //    Some(listener) => {
        //        listener.send(Event::BreakpointPending).wait().unwrap();
        //    }
        //    None => {}
        //}
    }

    fn jump_to_position(&mut self, file: String, line: u64) {
        jump_to_position(&file, line);
    }

    fn jump_to_unknown_position(&mut self) {
        log_msg(LogLevel::WARN, "Stopped at unknown position");
    }

    fn printed_variable(&mut self, variable: String, variable_type: String, data: &str) {
        // let mut start = 1;

        // while &data[start..start + 1] != ")" {
        //     start += 1;
        // }
        // while &data[start..start + 1] != "=" {
        //     start += 1;
        // }
        // start += 2;

        // // TODO: Need a better way of doing this to strip of the last \n,
        // // it's possible one day we'll screw the UTF-8 pooch here.
        // let value = data[start..data.len() - 1].to_string();

        // match self.listeners.remove(&Listener::PrintVariable) {
        //     Some(listener) => {
        //         let variable = Variable::new(variable);
        //         let value = VariableValue::new(variable_type, value);
        //         listener
        //             .send(Event::PrintVariable(variable, value))
        //             .wait()
        //             .unwrap();
        //     }
        //     None => {}
        // }
    }

    fn process_not_running(&self) {
        log_msg(LogLevel::WARN, "program not running");
    }

    fn variable_not_found(&mut self, variable: String) {
        //match self.listeners.remove(&Listener::PrintVariable) {
        //    Some(listener) => {
        //        let variable = Variable::new(variable);
        //        listener
        //            .send(Event::VariableNotFound(variable))
        //            .wait()
        //            .unwrap();
        //    }
        //    None => {}
        //}
    }
}
