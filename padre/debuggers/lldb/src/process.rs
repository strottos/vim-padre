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
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to LLDB for processing
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    ProcessLaunching,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    UnknownBreakpoint,
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
    Custom,
}

/// Current status of LLDB
#[derive(Debug, Clone, PartialEq)]
pub enum LLDBStatus {
    NotLaunched,
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
    lldb_process: Option<Child>,
    lldb_status: Arc<Mutex<LLDBStatus>>,
    lldb_stdin_tx: Sender<Bytes>,
}

impl LLDBProcess {
    /// Create and setup LLDB
    ///
    /// Includes spawning the LLDB process and all the relevant stdio handlers. In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read output from LLDB;
    /// - Sets up a thread to read stdin and forward it onto LLDB stdin;
    /// - Checks that LLDB and the program to be ran both exist, otherwise panics.
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        let mut lldb_process = check_and_spawn_process(vec![debugger_cmd], run_cmd);

        let lldb_status = Arc::new(Mutex::new(LLDBStatus::NotLaunched));

        LLDBProcess::setup_stdout(
            lldb_process
                .stdout()
                .take()
                .expect("LLDB process did not have a handle to stdout"),
            lldb_status,
        );
        let stdin_tx = LLDBProcess::setup_stdin(
            lldb_process
                .stdin()
                .take()
                .expect("Python process did not have a handle to stdin"),
        );

        LLDBProcess {
            lldb_process: Some(lldb_process),
            lldb_status,
            lldb_stdin_tx: stdin_tx,
        }
    }

    pub fn teardown(&mut self) {
        self.lldb_process = None;
    }

    pub fn get_status(&self) -> LLDBStatus {
        self.lldb_status.lock().unwrap().clone()
    }

    /// Send a message to write to stdin
    fn write_stdin(&mut self, bytes: Bytes) {
        let mut lldb_stdin_tx = self.lldb_stdin_tx.clone();
        tokio::spawn(async move {
            lldb_stdin_tx
                .send(bytes)
                .map(move |_| {})
                .await;
        });
    }

    // pub fn send_msg(&mut self, message: Message) {
    //     let msg_bytes = match message.clone() {
    //         Message::ProcessLaunching => Bytes::from("process launch\n"),
    //         Message::Breakpoint(fl) => Bytes::from(format!(
    //             "breakpoint set --file {} --line {}\n",
    //             fl.name(),
    //             fl.line_num()
    //         )),
    //         Message::UnknownBreakpoint => unreachable!(),
    //         Message::StepIn => Bytes::from("thread step-in\n"),
    //         Message::StepOver => Bytes::from("thread step-over\n"),
    //         Message::Continue => Bytes::from("thread continue\n"),
    //         Message::PrintVariable(v) => Bytes::from(format!("frame variable {}\n", v.name())),
    //     };

    //     self.write_stdin(msg_bytes);
    // }

    // /// Adds a Sender object that gets awoken when we are listening.
    // ///
    // /// Should only add a sender when we're about to go into or currently in the
    // /// processing status otherwise this will never wake up.
    // pub fn add_awakener(&mut self, sender: Sender<bool>) {
    //     self.analyser.lock().unwrap().add_awakener(sender);
    // }

    // /// Drop the awakener
    // pub fn drop_awakener(&mut self) {
    //     self.analyser.lock().unwrap().drop_awakener();
    // }

    // pub fn is_process_running(&self) -> bool {
    //     self.analyser.lock().unwrap().is_process_running()
    // }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(mut child_stdin: ChildStdin) -> Sender<Bytes> {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(32);
        let mut tx = stdin_tx.clone();

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
                }

                tx.send(buf).await.unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(text) = stdin_rx.next().await {
                io::stdout().write_all(&text).unwrap();
                match child_stdin.write(&text).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Writing stdin err e: {}", e);
                    }
                };
            }
        });

        stdin_tx
    }

    /// Perform setup of reading LLDB stdout, analysing it and writing it back to stdout.
    fn setup_stdout(stdout: ChildStdout, lldb_status: Arc<Mutex<LLDBStatus>>) {
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            let mut analyser = Analyser::new(lldb_status);
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                io::stdout().flush().unwrap();
                analyser.analyse_output(&text[..]);
            }
        });
    }
}

#[derive(Debug)]
pub struct Analyser {
    lldb_status: Arc<Mutex<LLDBStatus>>,
    stdout: String,
    process_pid: Option<u64>,
    awakener: Option<Sender<bool>>,
}

impl Analyser {
    pub fn new(lldb_status: Arc<Mutex<LLDBStatus>>) -> Self {
        Analyser {
            lldb_status,
            stdout: "".to_string(),
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

    pub fn analyse_output(&mut self, s: &str) {
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
            static ref RE_VARIABLE_NOT_FOUND: Regex =
                Regex::new("error: no variable named '([^']*)' found in this frame$").unwrap();
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

            for cap in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
                let variable = cap[1].to_string();
                self.variable_not_found(variable);
            }
        }

        self.clear_analyser();
    }

    fn set_listening(&mut self) {
        {
            let status = self.lldb_status.lock().unwrap();
            *status = LLDBStatus::Listening;
        }
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
        let status = *self.lldb_status.lock().unwrap();
        match status {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_startup() {
        // analyser.analyse_stdout("> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n");
        // assert_eq!(
        //     analyser.get_status(),
        //     PDBStatus::Processing(Message::Launching)
        // );
        // analyser.analyse_stdout("(Pdb) ");
        // assert_eq!(analyser.get_status(), PDBStatus::Listening);
    }
}
