//! lldb process handler
//!
//! This module performs the basic setup of and interfacing with LLDB. It will
//! analyse the output of the text and work out what is happening then.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use padre_core::debugger::{FileLocation, Variable};
use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
use padre_core::util::{check_and_spawn_process, read_output};

use bytes::Bytes;
use futures::prelude::*;
use regex::Regex;
use tokio::io::{stdin, BufReader};
use tokio::prelude::*;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to LLDB for processing
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    LLDBLaunching,
    LLDBSetup,
    ProcessLaunching,
    Breakpoint(FileLocation),
    // Unbreakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
    Custom,
}

/// Current status of LLDB
#[derive(Clone, Debug, PartialEq)]
pub enum LLDBStatus {
    NotRunning,
    Listening,
    Processing(Message),
}

/// Main handler for spawning the LLDB process
#[derive(Debug)]
pub struct LLDBProcess {
    lldb_process: Option<Child>,
    lldb_stdin_tx: mpsc::Sender<Bytes>,
    analyser: Arc<Mutex<LLDBAnalyser>>,
}

impl LLDBProcess {
    /// Create and setup LLDB
    ///
    /// Includes spawning the LLDB process and all the relevant stdio handlers. In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read output from LLDB;
    /// - Sets up a thread to read stdin and forward it onto LLDB stdin;
    /// - Checks that LLDB and the program to be ran both exist, otherwise panics.
    pub fn new(
        debugger_cmd: String,
        run_cmd: Vec<String>,
        tx_done: Option<oneshot::Sender<bool>>,
    ) -> Self {
        let analyser = Arc::new(Mutex::new(LLDBAnalyser::new()));

        analyser
            .lock()
            .unwrap()
            .analyse_message(Message::LLDBLaunching, tx_done);

        let mut lldb_process = check_and_spawn_process(vec![debugger_cmd], run_cmd);

        LLDBProcess::setup_stdout(
            lldb_process
                .stdout
                .take()
                .expect("LLDB process did not have a handle to stdout"),
            analyser.clone(),
        );
        let lldb_stdin_tx = LLDBProcess::setup_stdin(
            lldb_process
                .stdin
                .take()
                .expect("Python process did not have a handle to stdin"),
        );

        LLDBProcess {
            lldb_process: Some(lldb_process),
            lldb_stdin_tx,
            analyser,
        }
    }

    pub fn stop(&mut self) {
        match self.lldb_process.take() {
            Some(mut p) => {
                p.kill().unwrap();
            }
            None => {}
        };
    }

    /// Send a message to write to stdin
    pub fn send_msg(&mut self, message: Message, tx_done: Option<oneshot::Sender<bool>>) {
        let mut lldb_stdin_tx = self.lldb_stdin_tx.clone();
        let analyser = self.analyser.clone();

        let msg = match &message {
            Message::LLDBSetup => vec![
                Bytes::from("settings set stop-line-count-after 0\n"),
                Bytes::from("settings set stop-line-count-before 0\n"),
                Bytes::from("settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"),
                Bytes::from("breakpoint set --name main\n"),
            ],
            Message::ProcessLaunching => vec![Bytes::from("process launch\n")],
            Message::Breakpoint(fl) => vec![Bytes::from(format!(
                "breakpoint set --file {} --line {}\n",
                fl.name(),
                fl.line_num()
            ))],
            Message::StepIn => vec![Bytes::from("thread step-in\n")],
            Message::StepOver => vec![Bytes::from("thread step-over\n")],
            Message::Continue => vec![Bytes::from("thread continue\n")],
            Message::PrintVariable(v) => vec![Bytes::from(format!("frame variable {}\n", v.name()))],
            _ => unreachable!(),
        };

        tokio::spawn(async move {
            // TODO: Interrupt, set and then carry on as before?
            //
            // Something like:
            // let (tx, rx) = oneshot::channel();
            //
            // match self.analyser.lock().unwrap().get_status() {
            //     NotRunning => {},
            //     Listening => {},
            //     Processing(Message) => {
            //         analyser.lock().unwrap().analyse_message(message, Some(tx));
            //         rx.await.unwrap();
            //     },
            // }

            for b in msg {
                let (tx, rx) = oneshot::channel();

                analyser
                    .lock()
                    .unwrap()
                    .analyse_message(message.clone(), Some(tx));

                lldb_stdin_tx.send(b).map(move |_| {}).await;

                rx.await.unwrap();
            }

            match tx_done {
                Some(tx) => {
                    tx.send(true).unwrap();
                }
                _ => {}
            }
        });
    }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(mut child_stdin: ChildStdin) -> mpsc::Sender<Bytes> {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(32);
        let mut tx = stdin_tx.clone();

        tokio::spawn(async move {
            let tokio_stdin = stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            while let Some(line) = reader.next().await {
                let buf = line.unwrap().freeze();
                tx.send(buf).await.unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(text) = stdin_rx.next().await {
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
    fn setup_stdout(stdout: ChildStdout, analyser: Arc<Mutex<LLDBAnalyser>>) {
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                io::stdout().flush().unwrap();
                analyser.lock().unwrap().analyse_stdout(&text[..]);
            }
        });
    }
}

#[derive(Debug)]
pub struct LLDBAnalyser {
    status: LLDBStatus,
    process_pid: Option<u64>,
    awakener: Option<oneshot::Sender<bool>>,
    // For keeping track of the variable that we were told to print
    variable_output: String,
}

impl LLDBAnalyser {
    pub fn new() -> Self {
        LLDBAnalyser {
            status: LLDBStatus::NotRunning,
            variable_output: "".to_string(),
            awakener: None,
            process_pid: None,
        }
    }

    /// Check the current status, either not running (None), running something
    /// (Processing) or listening for a message on LLDB (Listening).
    pub fn get_status(&mut self) -> LLDBStatus {
        self.status.clone()
    }

    // TODO: Find out why I need this?
    #[allow(unused_mut)]
    pub fn analyse_stdout(&mut self, s: &str) {
        // Check process running first
        let mut process_running = true;

        for line in s.split("\r\n") {
            if self.check_process_not_running(line) {
                process_running = false;
            }

            if !process_running && line == "(lldb) " {
                self.set_listening();
                return;
            }
        }

        // Then check if we're printing a variable as we bail if not all output is available
        // straight away
        match self.get_status() {
            LLDBStatus::Processing(msg) => match msg {
                Message::PrintVariable(var) => {
                    let mut from = 0;
                    let mut to = s.len();

                    let print_cmd_size = 15 + var.name().len();
                    if to >= print_cmd_size + 2
                        && &s[0..print_cmd_size] == &format!("frame variable {}", var.name())
                    {
                        // 2 extra for \r\n
                        from += print_cmd_size + 2;
                    }

                    self.variable_output += &s[from..to];

                    let output = &self.variable_output[..];
                    let mut to = output.len();

                    let lldb_prompt_length = "\r\n(lldb) ".len();
                    if to >= lldb_prompt_length
                        && &self.variable_output[to - lldb_prompt_length..to] == "\r\n(lldb) "
                    {
                        to -= lldb_prompt_length;
                    }

                    self.variable_output = output[0..to].to_string();
                }
                _ => {}
            },
            _ => {}
        }

        // Then check everything else
        for line in s.split("\r\n") {
            if line == "(lldb) " {
                match self.get_status() {
                    LLDBStatus::Processing(msg) => match msg {
                        Message::PrintVariable(var) => {
                            if self.variable_output
                                == format!(
                                    "error: no variable named '{}' found in this frame",
                                    var.name()
                                )
                            {
                                log_msg(
                                    LogLevel::WARN,
                                    &format!("variable '{}' doesn't exist here", var.name()),
                                );
                            } else {
                                log_msg(LogLevel::INFO, &self.variable_output);
                            }
                            self.variable_output = "".to_string();
                        }
                        _ => {}
                    },
                    _ => {}
                }

                self.set_listening();
                return;
            }

            match self.get_status() {
                LLDBStatus::Listening => self.status = LLDBStatus::Processing(Message::Custom),
                _ => {}
            }

            match self.get_status() {
                LLDBStatus::Processing(msg) => match msg {
                    Message::LLDBSetup | Message::Breakpoint(_) => {
                        self.check_breakpoint(line);
                    }
                    Message::ProcessLaunching
                    | Message::StepIn
                    | Message::StepOver
                    | Message::Continue => {
                        self.check_location(line);
                        self.check_process_exited(line);
                    }
                    Message::Custom => {
                        self.check_breakpoint(line);
                        self.check_location(line);
                        self.check_process_exited(line);
                    }
                    _ => {}
                },
                _ => {}
            };
        }
    }

    fn set_listening(&mut self) {
        self.status = LLDBStatus::Listening;
        match self.awakener.take() {
            Some(tx) => {
                tx.send(true).unwrap();
            }
            None => {}
        };
    }

    /// Sets up the analyser ready for analysing the message.
    ///
    /// It sets the status of the analyser to Processing for that message and if given
    /// it marks the analyser to send a message to `tx_done` to indicate when the
    /// message is processed.
    pub fn analyse_message(&mut self, msg: Message, tx_done: Option<oneshot::Sender<bool>>) {
        self.status = LLDBStatus::Processing(msg);
        self.awakener = tx_done;
    }

    fn check_breakpoint(&mut self, line: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex = Regex::new(
                "Breakpoint (\\d+): where = .* at (.*):(\\d+):\\d+, address = 0x[0-9a-f]*$"
            )
            .unwrap();
            static ref RE_BREAKPOINT_2: Regex =
                Regex::new("Breakpoint (\\d+): where = .* at (.*):(\\d+), address = 0x[0-9a-f]*$")
                    .unwrap();
            static ref RE_BREAKPOINT_PENDING: Regex =
                Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
        }

        for cap in RE_BREAKPOINT.captures_iter(line) {
            let file = cap[2].to_string();
            let line = cap[3].parse::<u64>().unwrap();
            log_msg(
                LogLevel::INFO,
                &format!("Breakpoint set at file {} and line number {}", file, line),
            );
            return;
        }

        for cap in RE_BREAKPOINT_2.captures_iter(line) {
            let file = cap[2].to_string();
            let line = cap[3].parse::<u64>().unwrap();
            log_msg(
                LogLevel::INFO,
                &format!("Breakpoint set at file {} and line number {}", file, line),
            );
            return;
        }

        for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
            log_msg(LogLevel::INFO, &format!("Breakpoint pending"));
        }
    }

    fn check_location(&mut self, line: &str) {
        lazy_static! {
            static ref RE_STOPPED_AT_POSITION: Regex = Regex::new(" *frame #\\d.*$").unwrap();
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
        }

        for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
            let mut found = false;
            for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
                found = true;
                let file = cap[1].to_string();
                let line = cap[2].parse::<u64>().unwrap();
                jump_to_position(&file, line);
            }

            if !found {
                log_msg(LogLevel::WARN, "Stopped at unknown position");
            }
        }
    }

    fn check_process_exited(&mut self, line: &str) {
        lazy_static! {
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$")
                    .unwrap();
        }

        for cap in RE_PROCESS_EXITED.captures_iter(line) {
            let pid = cap[1].parse::<u64>().unwrap();
            let exit_code = cap[2].parse::<i64>().unwrap();
            self.process_pid = None;
            signal_exited(pid, exit_code);
        }
    }

    fn check_process_not_running(&mut self, line: &str) -> bool {
        lazy_static! {
            static ref RE_PROCESS_NOT_RUNNING: Regex =
                Regex::new("^error: invalid process$").unwrap();
        }

        for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
            log_msg(LogLevel::WARN, "No process running");
            return true;
        }

        false
    }
}
