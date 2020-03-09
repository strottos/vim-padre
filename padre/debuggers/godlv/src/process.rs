//! Delve process handler
//!
//! This module performs the basic setup of and interfacing with Delve. It will
//! analyse the output of the text and work out what is happening then.

use std::env;
use std::io::{self, Write};
use std::process::{Stdio, exit};
use std::sync::{Arc, Mutex};

use padre_core::debugger::{FileLocation, Variable};
use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
use padre_core::util::{check_and_spawn_process, read_output};

use bytes::Bytes;
use futures::prelude::*;
use futures::StreamExt;
use regex::Regex;
use tokio::io::{stdin, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to Delve for processing
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Message {
    DlvLaunching,
    DlvSetup,
    ProcessLaunching,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    PrintVariable(Variable),
    Custom(Bytes),
}

/// Current status of Delve
#[derive(Debug, Clone, PartialEq)]
pub enum DelveStatus {
    NotRunning,
    Listening,
    Processing(Message),
}

/// Main handler for spawning the `delve` process
#[derive(Debug)]
pub struct DlvProcess {
    dlv_process: Option<Child>,
    dlv_stdin_tx: mpsc::Sender<Bytes>,
    analyser: Arc<Mutex<DlvAnalyser>>,
}

impl DlvProcess {
    /// Create and setup a new `delve` process
    pub fn new(
        debugger_cmd: String,
        run_cmd: Vec<String>,
        tx_done: Option<oneshot::Sender<bool>>,
    ) -> Self {
        let analyser = Arc::new(Mutex::new(DlvAnalyser::new()));

        analyser
            .lock()
            .unwrap()
            .analyse_message(Message::DlvLaunching, tx_done);

        let mut dlv_process = check_and_spawn_process(vec![debugger_cmd, "exec".to_string()], run_cmd);

        DlvProcess::setup_stdout(
            dlv_process
                .stdout
                .take()
                .expect("Python process did not have a handle to stdout"),
            analyser.clone(),
        );
        let dlv_stdin_tx = DlvProcess::setup_stdin(
            dlv_process
                .stdin
                .take()
                .expect("Python process did not have a handle to stdin"),
            analyser.clone(),
        );

        DlvProcess {
            dlv_process: Some(dlv_process),
            dlv_stdin_tx,
            analyser,
        }
    }

    pub fn stop(&mut self) {
        match self.dlv_process.take() {
            Some(mut p) => {
                p.kill().unwrap();
            }
            None => {}
        };
    }

    /// Send a message to write to stdin
    pub fn send_msg(&mut self, message: Message, tx_done: Option<oneshot::Sender<bool>>) {
        let mut dlv_stdin_tx = self.dlv_stdin_tx.clone();
        let analyser = self.analyser.clone();

        let msg = match &message {
            Message::DlvSetup => vec![Bytes::from("break main.main\n"), Bytes::from("restart\n"), Bytes::from("continue\n")],
            Message::ProcessLaunching => vec![Bytes::from("restart\n")],
            Message::Breakpoint(fl) => {
                vec![Bytes::from(format!("break {}:{}\n", fl.name(), fl.line_num()))]
            }
            Message::Unbreakpoint(fl) => {
                vec![Bytes::from(format!("clear {}:{}\n", fl.name(), fl.line_num()))]
            },
            Message::StepIn => vec![Bytes::from("si\n")],
            Message::StepOver => vec![Bytes::from("next\n")],
            Message::Continue => vec![Bytes::from("continue\n")],
            Message::PrintVariable(v) => vec![Bytes::from(format!("print {}\n", v.name()))],
            Message::Custom(s) => vec![s.clone()],
            _ => unreachable!(),
        };

        tokio::spawn(async move {
            for b in msg {
                let (tx, rx) = oneshot::channel();

                analyser
                    .lock()
                    .unwrap()
                    .analyse_message(message.clone(), Some(tx));

                dlv_stdin_tx.send(b).map(move |_| {}).await;

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
    fn setup_stdin(mut child_stdin: ChildStdin, analyser: Arc<Mutex<DlvAnalyser>>) -> mpsc::Sender<Bytes> {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(1);
        let mut tx = stdin_tx.clone();

        tokio::spawn(async move {
            let tokio_stdin = stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            let analyser = analyser.clone();
            while let Some(line) = reader.next().await {
                let buf = line.unwrap().freeze();
                {
                    let mut analyser_lock = analyser.lock().unwrap();
                    let status = analyser_lock.status.clone();
                    match status {
                        DelveStatus::Listening => {
                            analyser_lock.status = DelveStatus::Processing(Message::Custom(buf.clone()));
                        },
                        _ => {}
                    }
                }
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

    /// Perform setup of reading `delve` stdout, analysing it and writing it back to stdout.
    fn setup_stdout(stdout: ChildStdout, analyser: Arc<Mutex<DlvAnalyser>>) {
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
                io::stdout().flush().unwrap();
                analyser.lock().unwrap().analyse_output(&text[..]);
            }
        });
    }
}

#[derive(Debug)]
pub struct DlvAnalyser {
    status: DelveStatus,
    awakener: Option<oneshot::Sender<bool>>,
    // For keeping track of the variable that we were told to print
    variable_output: String,
}

impl DlvAnalyser {
    pub fn new() -> Self {
        DlvAnalyser {
            status: DelveStatus::NotRunning,
            awakener: None,
            variable_output: "".to_string(),
        }
    }

    pub fn analyse_output(&mut self, s: &str) {
        for line in s.split("\r\n") {
            if line == "(dlv) " {
                match &self.status {
                    DelveStatus::Processing(msg) => {
                        match msg {
                            Message::PrintVariable(var) => {
                                let to = if self.variable_output.len() < 1 {
                                    0
                                } else {
                                    self.variable_output.len() - 1
                                };

                                log_msg(LogLevel::INFO, &format!("{}={}", var.name(), &self.variable_output[0..to]));
                                self.variable_output = "".to_string();
                            },
                            _ => {},
                        };
                    },
                    _ => {}
                };

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

            match &self.status {
                DelveStatus::Processing(msg) => {
                    match msg {
                        Message::DlvSetup => {
                            self.check_process_launched(line);
                            self.check_position(line);
                            self.check_exited(line);
                        },
                        Message::Breakpoint(_) => {
                            self.check_breakpoint(line);
                        },
                        Message::StepIn | Message::StepOver | Message::Continue | Message::Custom(_) => {
                            self.check_position(line);
                            self.check_exited(line);
                        },
                        Message::PrintVariable(var) => {
                            if line != &format!("print {}", var.name()) && line != "" {
                                self.variable_output += &format!("{}\n", line);
                            }
                        },
                        _ => {},
                    };
                },
                _ => {}
            };
        }

        match &self.status {
            DelveStatus::Processing(msg) => {
                match msg {
                    Message::PrintVariable(var) => {
                        if s != &format!("print {}\r\n", var.name()) {
                            log_msg(LogLevel::INFO, s);
                        }
                    },
                    _ => {},
                };
            },
            _ => {}
        };
    }

    /// Sets up the analyser ready for analysing the message.
    ///
    /// It sets the status of the analyser to Processing for that message and if given
    /// it marks the analyser to send a message to `tx_done` to indicate when the
    /// message is processed.
    pub fn analyse_message(&mut self, msg: Message, tx_done: Option<oneshot::Sender<bool>>) {
        self.status = DelveStatus::Processing(msg);
        self.awakener = tx_done;
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

    fn check_process_launched(&self, line: &str) {
        lazy_static! {
            static ref RE_LAUNCHED: Regex =
                Regex::new("^.*Process restarted with PID (\\d*)$").unwrap();
        }

        for cap in RE_LAUNCHED.captures_iter(line) {
            let pid = cap[1].parse::<u64>().unwrap();
            log_msg(
                LogLevel::INFO,
                &format!("Process launched with pid: {}", pid),
            );
        }
    }

    fn check_exited(&self, line: &str) {
        lazy_static! {
            static ref RE_EXITED: Regex =
                Regex::new("^.*Process (\\d*) has exited with status (-*\\d*)$").unwrap();
        }

        for cap in RE_EXITED.captures_iter(line) {
            let pid = cap[1].parse::<u64>().unwrap();
            let exit_code = cap[2].parse::<i64>().unwrap();
            signal_exited(pid, exit_code);
        }
    }
}
