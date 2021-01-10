//! lldb client debugger
//!
//! The main LLDB Debugger entry point. Handles listening for instructions and
//! communicating through an `LLDBProcess` object.

use std::process::exit;
use std::time::Instant;

use super::process::{LLDBProcess, Message};
use padre_core::debugger::{DebuggerCmd, FileLocation, Variable};
use padre_core::server::{LogLevel, Notification, PadreError, PadreErrorKind};
use padre_core::util::log_msg;
use padre_core::Result;

use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout_at;

#[derive(Debug)]
pub struct LLDBDebugger {
    queue_rx: mpsc::Receiver<(
        DebuggerCmd,
        Instant,
        oneshot::Sender<Result<serde_json::Value>>,
    )>,
    process: LLDBProcess,
    notifier_tx: mpsc::Sender<Notification>,
}

impl LLDBDebugger {
    pub fn new(
        debugger_cmd: String,
        run_cmd: Vec<String>,
        queue_rx: mpsc::Receiver<(
            DebuggerCmd,
            Instant,
            oneshot::Sender<Result<serde_json::Value>>,
        )>,
        notifier_tx: mpsc::Sender<Notification>,
    ) -> LLDBDebugger {
        let process = LLDBProcess::new(debugger_cmd, run_cmd, notifier_tx.clone());

        LLDBDebugger {
            queue_rx,
            process,
            notifier_tx,
        }
    }

    /// Perform any initial setup including starting LLDB and setting up the stdio analyser stuff
    /// - startup lldb and setup the stdio analyser
    /// - perform initial setup so we can analyse LLDB properly
    #[allow(unreachable_patterns)]
    pub async fn start(&mut self) {
        self.process.startup().await;

        while let Some((cmd, timeout, tx)) = self.queue_rx.recv().await {
            match cmd {
                DebuggerCmd::Run => self.run(timeout, tx),
                DebuggerCmd::Breakpoint(fl) => self.breakpoint(fl, timeout, tx),
                DebuggerCmd::Unbreakpoint(fl) => self.unbreakpoint(fl, timeout, tx),
                DebuggerCmd::StepIn(count) => self.step_in(timeout, count, tx),
                DebuggerCmd::StepOver(count) => self.step_over(timeout, count, tx),
                DebuggerCmd::Continue => self.continue_(timeout, tx),
                DebuggerCmd::Print(v) => self.print(v, timeout, tx),
                _ => {
                    tx.send(Err(PadreError::new(
                        PadreErrorKind::DebuggerError,
                        "Bad command".to_string(),
                        format!("Got a command that isn't supported '{:?}'", cmd),
                    )))
                    .unwrap();
                }
            };
        }

        exit(0);
    }

    pub async fn stop(&mut self) {
        self.process.stop().await;
        exit(0);
    }

    fn run(&mut self, timeout: Instant, tx_done: oneshot::Sender<Result<serde_json::Value>>) {
        log_msg(
            self.notifier_tx.clone(),
            LogLevel::INFO,
            "Launching process",
        );

        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let started = Instant::now();
            match timeout_at(tokio::time::Instant::from_std(timeout), rx).await {
                Ok(ret) => {
                    tx_done.send(ret.unwrap()).unwrap();
                }
                Err(_) => {
                    tx_done
                        .send(Err(PadreError::new(
                            PadreErrorKind::DebuggerError,
                            "Timed out spawning process".to_string(),
                            format!(
                                "Process spawning timed out after {:?}",
                                timeout.duration_since(started)
                            ),
                        )))
                        .unwrap();
                }
            }
        });

        self.process.send_msg(Message::ProcessLaunching, Some(tx));
    }

    fn breakpoint(
        &mut self,
        file_location: FileLocation,
        timeout: Instant,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        log_msg(
            self.notifier_tx.clone(),
            LogLevel::INFO,
            &format!(
                "Setting breakpoint in file {} at line number {}",
                file_location.name(),
                file_location.line_num()
            ),
        );

        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let started = Instant::now();
            match timeout_at(tokio::time::Instant::from_std(timeout), rx).await {
                Ok(ret) => {
                    tx_done.send(ret.unwrap()).unwrap();
                }
                Err(_) => {
                    tx_done
                        .send(Err(PadreError::new(
                            PadreErrorKind::DebuggerError,
                            "Timed out setting breakpoint".to_string(),
                            format!(
                                "Breakpoint setting timed out after {:?}",
                                timeout.duration_since(started)
                            ),
                        )))
                        .unwrap();
                }
            }
        });

        self.process
            .send_msg(Message::Breakpoint(file_location), Some(tx));
    }

    fn unbreakpoint(
        &mut self,
        file_location: FileLocation,
        timeout: Instant,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let started = Instant::now();
            match timeout_at(tokio::time::Instant::from_std(timeout), rx).await {
                Ok(ret) => {
                    tx_done.send(ret.unwrap()).unwrap();
                }
                Err(_) => {
                    tx_done
                        .send(Err(PadreError::new(
                            PadreErrorKind::DebuggerError,
                            "Timed out removing breakpoint".to_string(),
                            format!(
                                "Breakpoint removing timed out after {:?}",
                                timeout.duration_since(started)
                            ),
                        )))
                        .unwrap();
                }
            }
        });

        self.process
            .send_msg(Message::Unbreakpoint(file_location), Some(tx));
    }

    fn step_in(
        &mut self,
        _timeout: Instant,
        count: u64,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        self.process.send_msg(Message::StepIn(count), None);

        tx_done.send(Ok(serde_json::json!({}))).unwrap();
    }

    fn step_over(
        &mut self,
        _timeout: Instant,
        count: u64,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        self.process.send_msg(Message::StepOver(count), None);

        tx_done.send(Ok(serde_json::json!({}))).unwrap();
    }

    fn continue_(
        &mut self,
        _timeout: Instant,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        self.process.send_msg(Message::Continue, None);

        tx_done.send(Ok(serde_json::json!({}))).unwrap();
    }

    fn print(
        &mut self,
        variable: Variable,
        timeout: Instant,
        tx_done: oneshot::Sender<Result<serde_json::Value>>,
    ) {
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let started = Instant::now();
            match timeout_at(tokio::time::Instant::from_std(timeout), rx).await {
                Ok(ret) => {
                    tx_done.send(ret.unwrap()).unwrap();
                }
                Err(_) => {
                    tx_done
                        .send(Err(PadreError::new(
                            PadreErrorKind::DebuggerError,
                            "Timed out printing variable".to_string(),
                            format!(
                                "Printing variable timed out after {:?}",
                                timeout.duration_since(started)
                            ),
                        )))
                        .unwrap();
                }
            }
        });

        self.process
            .send_msg(Message::PrintVariable(variable.clone()), Some(tx));
    }
}
