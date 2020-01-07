//! Delve process handler
//!
//! This module performs the basic setup of and interfacing with Delve. It will
//! analyse the output of the text and work out what is happening then.

use std::env;
use std::io::{self, Write};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
use padre_core::server::{FileLocation, Variable};
use padre_core::util::read_output;

use bytes::Bytes;
use futures::prelude::*;
use futures::StreamExt;
use regex::Regex;
use tokio::io::{stdin, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to Delve for processing
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
    Custom(String),
}

/// Current status of Delve
#[derive(Debug, Clone, PartialEq)]
pub enum DelveStatus {
    None,
    Listening,
    Processing(Message),
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

    pub fn run(&mut self) {
        let debugger_cmd = self.debugger_cmd.take().unwrap();
        let run_cmd = self.run_cmd.take().unwrap();

        let mut pty_wrapper = env::current_exe().unwrap();
        pty_wrapper.pop();
        pty_wrapper.pop();
        pty_wrapper.pop();
        pty_wrapper.push("ptywrapper.py");

        let mut process = Command::new(pty_wrapper)
            .arg(&debugger_cmd)
            .arg("debug")
            .arg("--")
            .args(&run_cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn debugger");

        self.setup_stdout(
            process
                .stdout()
                .take()
                .expect("Python process did not have a handle to stdout"),
        );
        self.setup_stdin(
            process
                .stdin()
                .take()
                .expect("Python process did not have a handle to stdin"),
        );

        log_msg(
            LogLevel::INFO,
            &format!("Process launched with pid: {}", process.id()),
        );

        self.process = Some(process);
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
                },
                Message::StepIn => Bytes::from("si\n"),
                Message::StepOver => Bytes::from("step\n"),
                Message::Continue => Bytes::from("continue\n"),
                Message::PrintVariable(v) => Bytes::from(format!("print {}\n", v.name())),
                Message::Custom(s) => Bytes::from(format!("{}\n", s)),
            };

            analyser.lock().unwrap().analyse_message(message);

            tx.clone().unwrap().send(msg).map(move |_| {}).await
        });
    }

    /// Adds a Sender object that gets awoken when we are listening.
    ///
    /// Should only add a sender when we're about to go into or currently in the
    /// processing status otherwise this will never wake up.
    pub fn add_awakener(&self, sender: oneshot::Sender<bool>) {
        self.analyser.lock().unwrap().add_awakener(sender);
    }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(&mut self, mut child_stdin: ChildStdin) {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(1);
        self.stdin_tx = Some(stdin_tx.clone());

        tokio::spawn(async move {
            let tokio_stdin = stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            while let Some(line) = reader.next().await {
                let buf = line.unwrap().freeze();
                stdin_tx.clone().send(buf).await.unwrap();
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
    }

    /// Perform setup of reading Python stdout, analysing it and writing it back to stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        let analyser = self.analyser.clone();
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                io::stdout().flush().unwrap();
                analyser.lock().unwrap().analyse_output(&text);
            }
        });
    }
}

#[derive(Debug)]
pub struct Analyser {
    status: DelveStatus,
    awakener: Option<oneshot::Sender<bool>>,
}

impl Analyser {
    pub fn new() -> Self {
        Analyser {
            status: DelveStatus::None,
            awakener: None,
        }
    }

    /// Add the awakener to send a message to when we awaken
    pub fn add_awakener(&mut self, sender: oneshot::Sender<bool>) {
        self.awakener = Some(sender);
    }

    pub fn analyse_output(&mut self, s: &str) {
        for line in s.split("\r\n") {
            if line == "(dlv) " {
                self.status = DelveStatus::Listening;
                match self.awakener.take() {
                    Some(awakener) => {
                        tokio::spawn(async move {
                            awakener.send(true).unwrap();
                        });
                    },
                    None => {},
                }
            }

            self.check_breakpoint(line);
            self.check_position(line);
        }
    }

    fn check_breakpoint(&self, line: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex =
                Regex::new("^Breakpoint \\d* set at 0x[0-9a-fA-F]* for .* (.*):(\\d*)$").unwrap();
        }

        for cap in RE_BREAKPOINT.captures_iter(line) {
            let file = cap[1].to_string();
            let line = cap[2].parse::<u64>().unwrap();
            log_msg(
                LogLevel::INFO,
                &format!("Breakpoint set at file {} and line number {}", file, line),
            );
        }
    }

    fn check_position(&self, line: &str) {
        lazy_static! {
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^.*> .* (.*):(\\d*) \\(.*\\)$").unwrap();
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            let file = cap[1].to_string();
            let line = cap[2].parse::<u64>().unwrap();
            jump_to_position(&file, line);
        }
    }

    pub fn analyse_message(&mut self, msg: Message) {
        self.status = DelveStatus::Processing(msg);
    }
}
