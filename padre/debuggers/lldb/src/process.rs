//! lldb process handler
//!
//! This module performs the basic setup of and interfacing with LLDB. It will
//! analyse the output of the text and work out what is happening then.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use padre_core::debugger::{FileLocation, Variable};
use padre_core::server::{LogLevel, Notification, PadreError, PadreErrorKind};
use padre_core::util::{check_and_spawn_process, jump_to_position, log_msg};
use padre_core::Result;

use bytes::{BufMut, Bytes, BytesMut};
use futures::prelude::*;
use regex::Regex;
use tokio::io::{self as tokio_io, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to LLDB for processing
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    LLDBSetup,
    ProcessLaunching,
    Breakpoint(FileLocation),
    Unbreakpoint(FileLocation),
    // count
    StepIn(u64),
    // count
    StepOver(u64),
    Continue,
    PrintVariable(Variable),
    Custom,
}

/// Current status of LLDB
///
/// We start in Processing(LLDBSetup), then when LLDB has started up properly it moves to
/// `Listening`. Whenever a message is send to LLDB it changes to `Processing(message)` where
/// `message` is anything of type Message above. A Custom message indicates that it has been typed
/// in by the user and PADRE is unaware of what was typed (though will still pick up consequences
/// like line changing, etc).
#[derive(Clone, Debug, PartialEq)]
pub enum LLDBStatus {
    Listening,
    Processing((Message, Option<Bytes>)),
}

/// Main handler for spawning the LLDB process
///
/// The status contains the process id of any PID running (or None if there isn't one) and the
#[derive(Debug)]
pub struct LLDBProcess {
    lldb_process: Child,
    lldb_stdin_tx: mpsc::Sender<Bytes>,
    analyser: Arc<Mutex<LLDBAnalyser>>,
    notifier_tx: mpsc::Sender<Notification>,
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
        notifier_tx: mpsc::Sender<Notification>,
    ) -> Self {
        let mut lldb_process = match check_and_spawn_process(vec![debugger_cmd], run_cmd) {
            Ok(process) => process,
            Err(err) => panic!("Can't spawn LLDB: {:?}", err),
        };

        let analyser = Arc::new(Mutex::new(LLDBAnalyser::new(notifier_tx.clone())));

        // NB: Don't need stderr as it's taken from a process spawned with ptywrapper.py that will
        // wrap stderr into stdout.
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
            analyser.clone(),
        );

        LLDBProcess {
            lldb_process,
            lldb_stdin_tx,
            analyser,
            notifier_tx,
        }
    }

    pub async fn startup(&mut self) {
        match self.get_status() {
            LLDBStatus::Listening => {}
            _ => {
                let (tx, rx) = oneshot::channel();
                self.analyser.lock().unwrap().send_output_tx = Some(tx);
                rx.await.unwrap().unwrap();
            }
        };

        for msg in vec!(
            Bytes::from("settings set stop-line-count-after 0"),
            Bytes::from("settings set stop-line-count-before 0"),
            Bytes::from("settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n"),
            Bytes::from("breakpoint set --name main"),
        ) {
            let (tx, rx) = oneshot::channel();
            self.analyser
                .lock()
                .unwrap()
                .get_output(Message::LLDBSetup, msg.clone(), tx);

            self.lldb_stdin_tx.send(msg).await.unwrap();

            rx.await.unwrap().unwrap();
        }
    }

    pub async fn stop(&mut self) {
        self.lldb_process.kill().await.unwrap();
    }

    pub fn get_status(&self) -> LLDBStatus {
        self.analyser.lock().unwrap().lldb_status.clone()
    }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(
        mut child_stdin: ChildStdin,
        analyser: Arc<Mutex<LLDBAnalyser>>,
    ) -> mpsc::Sender<Bytes> {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(32);
        let tx = stdin_tx.clone();

        tokio::spawn(async move {
            let tokio_stdin = tokio_io::stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            while let Some(line) = reader.next().await {
                let buf = line.unwrap().freeze();
                analyser
                    .lock()
                    .unwrap()
                    .set_status(LLDBStatus::Processing((Message::Custom, Some(buf.clone()))));
                tx.send(buf).await.unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(text) = stdin_rx.recv().await {
                match child_stdin.write(&text).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Writing stdin err e: {}", e);
                    }
                };
                // Append \n if necessary
                let len = text.len();
                if &text[len - 1..len] != "\n".as_bytes() {
                    match child_stdin.write(&[10 as u8]).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Writing stdin err e: {}", e);
                        }
                    }
                }
            }
        });

        stdin_tx
    }

    /// Perform setup of reading LLDB stdout, analysing it and writing it back to stdout.
    fn setup_stdout(stdout: ChildStdout, analyser: Arc<Mutex<LLDBAnalyser>>) {
        tokio::spawn(async move {
            let mut reader = FramedRead::new(stdout, BytesCodec::new());
            while let Some(Ok(text)) = reader.next().await {
                analyser.lock().unwrap().handle_output(text);
            }
        });
    }

    /// Send a message to write to stdin
    pub fn send_msg(
        &mut self,
        message: Message,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let lldb_stdin_tx = self.lldb_stdin_tx.clone();
        let analyser = self.analyser.clone();

        let (_, process_pid) = analyser.lock().unwrap().get_details();

        let msg = match &message {
            Message::ProcessLaunching => {
                match process_pid {
                    Some(pid) => {
                        match tx_done {
                            Some(tx) => {
                                let res = Err(PadreError::new(
                                    PadreErrorKind::DebuggerError,
                                    "Process already running".to_string(),
                                    format!("Process with pid '{}' already running", pid),
                                ));
                                tx.send(res).unwrap();
                            }
                            None => {}
                        }
                        return;
                    }
                    None => {}
                }
                vec![Bytes::from("process launch")]
            }
            Message::Breakpoint(fl) => vec![Bytes::from(format!(
                "breakpoint set --file {} --line {}",
                fl.name(),
                fl.line_num()
            ))],
            Message::Unbreakpoint(fl) => return self.remove_breakpoint(fl.clone(), tx_done),
            Message::StepIn(count) => return self.step(Message::StepIn(*count), tx_done),
            Message::StepOver(count) => return self.step(Message::StepOver(*count), tx_done),
            Message::Continue => vec![Bytes::from("thread continue")],
            Message::PrintVariable(v) => return self.print_variable(v.clone(), tx_done),
            _ => unreachable!(),
        };

        tokio::spawn(async move {
            let res = Ok(serde_json::json!({}));

            for b in msg {
                let (tx, rx) = oneshot::channel();

                analyser
                    .lock()
                    .unwrap()
                    .get_output(message.clone(), b.clone(), tx);

                lldb_stdin_tx.send(b).map(move |_| {}).await;

                rx.await.unwrap().unwrap();
            }
            match tx_done {
                Some(tx) => {
                    tx.send(res).unwrap();
                }
                _ => {}
            }
        });
    }

    fn step(
        &self,
        message: Message,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let lldb_stdin_tx = self.lldb_stdin_tx.clone();
        let analyser = self.analyser.clone();

        let (bytes, count) = match message {
            Message::StepIn(count) => (Bytes::from("thread step-in"), count),
            Message::StepOver(count) => (Bytes::from("thread step-over"), count),
            _ => unreachable!(),
        };

        tokio::spawn(async move {
            analyser.lock().unwrap().reporting_location = false;

            for i in 0..count {
                let (tx, rx) = oneshot::channel();

                if i == count - 1 {
                    analyser.lock().unwrap().reporting_location = true;
                }

                analyser
                    .lock()
                    .unwrap()
                    .get_output(message.clone(), bytes.clone(), tx);

                lldb_stdin_tx.send(bytes.clone()).map(move |_| {}).await;

                rx.await.unwrap().unwrap();
            }

            match tx_done {
                Some(tx) => {
                    tx.send(Ok(serde_json::json!({}))).unwrap();
                }
                _ => {}
            }
        });
    }

    fn print_variable(
        &self,
        v: Variable,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let lldb_stdin_tx = self.lldb_stdin_tx.clone();
        let analyser = self.analyser.clone();

        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();

            let msg = Bytes::from(format!("frame variable {}", v.name()));

            analyser
                .lock()
                .unwrap()
                .get_output(Message::PrintVariable(v.clone()), msg.clone(), tx);

            lldb_stdin_tx.send(msg).map(move |_| {}).await;

            tx_done
                .unwrap()
                .send(get_variable_info(rx.await.unwrap(), &v))
                .unwrap();
        });
    }

    fn remove_breakpoint(
        &self,
        fl: FileLocation,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let lldb_stdin_tx = self.lldb_stdin_tx.clone();
        let analyser = self.analyser.clone();

        let notifier_tx = self.notifier_tx.clone();

        tokio::spawn(async move {
            let msg = Message::Unbreakpoint(fl.clone());

            let (tx, rx) = oneshot::channel();

            let bytes = Bytes::from("breakpoint list");

            analyser
                .lock()
                .unwrap()
                .get_output(msg.clone(), bytes.clone(), tx);

            lldb_stdin_tx.send(bytes).map(move |_| {}).await;

            // TODO: Errors
            let values = get_breakpoints_set_at(rx.await.unwrap(), &fl);

            for bkt_num in values {
                let (tx, rx) = oneshot::channel();

                let bytes = Bytes::from(format!("breakpoint delete {}", bkt_num));

                analyser
                    .lock()
                    .unwrap()
                    .get_output(msg.clone(), bytes.clone(), tx);

                lldb_stdin_tx.send(bytes).map(move |_| {}).await;

                rx.await.unwrap().unwrap();
            }

            log_msg(
                notifier_tx,
                LogLevel::INFO,
                &format!(
                    "Removed breakpoint in file {} and line number {}",
                    fl.name(),
                    fl.line_num()
                ),
            );

            match tx_done {
                Some(tx) => {
                    tx.send(Ok(serde_json::json!({}))).unwrap();
                }
                _ => {}
            }
        });
    }
}

fn get_variable_info(output: Result<String>, variable: &Variable) -> Result<serde_json::Value> {
    let details = output.unwrap();

    if details == "error: invalid process"
        || details
            == format!(
                "error: no variable named '{}' found in this frame",
                variable.name(),
            )
    {
        return Err(PadreError::new(
            PadreErrorKind::DebuggerError,
            "Variable not found".to_string(),
            format!("Variable '{}' not found", variable.name()),
        ));
    }

    let mut right_bracket_index = 2;
    if &details[0..1] != "(" {
        panic!("Can't understand printing variable output: {:?}", details,);
    }

    while &details[right_bracket_index - 1..right_bracket_index] != ")" {
        right_bracket_index += 1;
    }

    let type_ = details[1..right_bracket_index - 1].to_string();

    let mut equals_index = 2;

    while &details[equals_index - 1..equals_index] != "=" {
        equals_index += 1;
    }

    let name = details[right_bracket_index + 1..equals_index - 2].to_string();
    let value = details[equals_index + 1..].to_string();

    Ok(serde_json::json!({
        "variable": name,
        "type": type_,
        "value": value,
    }))
}

fn get_breakpoints_set_at(output: Result<String>, fl: &FileLocation) -> Vec<u32> {
    // TODO: Valid? Catches all?
    lazy_static! {
        static ref RE_BREAKPOINT: Regex =
            Regex::new("(\\d+): file = '(.*)', line = (\\d+), exact_match = 0, locations = 1$")
                .unwrap();
    }

    let details = output.unwrap();

    let mut ret = vec![];

    for line in details.split("\r\n") {
        for cap in RE_BREAKPOINT.captures_iter(line) {
            let breakpoint_num = cap[1].parse::<u32>().unwrap();
            let file = cap[2].to_string();
            let line = cap[3].parse::<u64>().unwrap();

            if fl == &FileLocation::new(file, line) {
                ret.push(breakpoint_num);
            }
        }
    }

    ret
}

/// Perform analysis on the output from the LLDB process and store data for when we need to access
/// it.
#[derive(Debug)]
pub struct LLDBAnalyser {
    lldb_status: LLDBStatus,
    process_pid: Option<String>,
    send_output_tx: Option<oneshot::Sender<Result<String>>>,
    notifier_tx: mpsc::Sender<Notification>,
    output: BytesMut,
    reporting_location: bool,
}

impl LLDBAnalyser {
    pub fn new(notifier_tx: mpsc::Sender<Notification>) -> Self {
        LLDBAnalyser {
            lldb_status: LLDBStatus::Processing((Message::LLDBSetup, None)),
            process_pid: None,
            send_output_tx: None,
            notifier_tx,
            output: BytesMut::new(),
            reporting_location: true,
        }
    }

    pub fn handle_output(&mut self, bytes: BytesMut) {
        let text = String::from_utf8_lossy(&bytes[..]).to_string();
        print!("{}", text);
        io::stdout().flush().unwrap();

        // Check if we're printing a variable as we record the data for later and only print when
        // we're done collecting output
        match &self.lldb_status {
            LLDBStatus::Processing((_, m)) => {
                match m {
                    Some(msg) => {
                        let mut from = 0;
                        let to = text.len();

                        if self.output.len() == 0
                            // 2 extra for \r\n
                            && to >= msg.len() + 2
                            && &text[0..msg.len()] == msg
                        {
                            from += msg.len() + 2;
                        }

                        self.output.put(&bytes[from..to]);
                    }
                    None => {}
                };
            }
            _ => {}
        }

        // Then check everything else
        for line in text.split("\r\n") {
            match &self.lldb_status {
                LLDBStatus::Processing((msg, _)) => match msg {
                    Message::Breakpoint(_) => {
                        self.check_breakpoint(line);
                    }
                    Message::ProcessLaunching => {
                        self.check_process_launched(line);
                        self.check_location(line);
                        self.check_process_exited(line);
                    }
                    Message::StepIn(_) | Message::StepOver(_) | Message::Continue => {
                        self.check_location(line);
                        self.check_process_exited(line);
                        self.check_process_and_thread_running(line);
                    }
                    Message::Custom => {
                        self.check_process_launched(line);
                        self.check_breakpoint(line);
                        self.check_location(line);
                        self.check_process_exited(line);
                    }
                    _ => {}
                },
                // Seems to be some bug in LLDB where sometimes it will still output this stuff
                // after it went back into listening mode.
                LLDBStatus::Listening => {
                    self.check_location(line);
                    self.check_process_exited(line);
                }
            };
        }

        match text.split("\r\n").last() {
            Some(l) => {
                if l == "(lldb) " {
                    match &self.lldb_status {
                        LLDBStatus::Processing(_) => {
                            let tx_option = self.send_output_tx.take();
                            match tx_option {
                                Some(tx) => {
                                    let mut to = self.output.len();
                                    let lldb_prompt_length = "\r\n(lldb) ".len();
                                    if to >= lldb_prompt_length
                                        && &self.output[to - lldb_prompt_length..to]
                                            == "\r\n(lldb) ".as_bytes()
                                    {
                                        to -= lldb_prompt_length;
                                    }

                                    let output =
                                        String::from_utf8_lossy(&self.output[0..to]).to_string();

                                    tx.send(Ok(output)).unwrap();
                                }
                                None => {}
                            }
                        }
                        _ => {}
                    }
                    self.output = BytesMut::new();
                    self.set_status(LLDBStatus::Listening);
                }
            }
            None => {}
        }
    }

    pub fn get_details(&self) -> (LLDBStatus, Option<String>) {
        (self.lldb_status.clone(), self.process_pid.clone())
    }

    pub fn set_status(&mut self, lldb_status: LLDBStatus) {
        match &self.lldb_status {
            LLDBStatus::Listening => {}
            _ => {
                match lldb_status {
                    LLDBStatus::Listening => {
                        let tx_option = self.send_output_tx.take();
                        match tx_option {
                            Some(tx) => {
                                tx.send(Ok("".to_string())).unwrap();
                            }
                            _ => {}
                        };
                    }
                    _ => {}
                };
            }
        }
        self.lldb_status = lldb_status;
    }

    /// Sets up the analyser ready for analysing the message.
    ///
    /// It sets the status of the analyser to Processing for that message and if given
    /// it marks the analyser to send a message to `tx_done` to indicate when the
    /// message is processed.
    pub fn get_output(
        &mut self,
        msg: Message,
        cmd: Bytes,
        tx_done: oneshot::Sender<Result<String>>,
    ) {
        self.lldb_status = LLDBStatus::Processing((msg, Some(cmd)));
        self.send_output_tx = Some(tx_done);
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
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Breakpoint set file={}, line={}", file, line),
            );
            return;
        }

        for cap in RE_BREAKPOINT_2.captures_iter(line) {
            let file = cap[2].to_string();
            let line = cap[3].parse::<u64>().unwrap();
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Breakpoint set file={}, line={}", file, line),
            );
            return;
        }

        for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Breakpoint pending"),
            );
        }
    }

    fn check_location(&mut self, line: &str) {
        if !self.reporting_location {
            return;
        }

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
                jump_to_position(self.notifier_tx.clone(), &file, line);
            }

            if !found {
                log_msg(
                    self.notifier_tx.clone(),
                    LogLevel::WARN,
                    "Stopped at unknown position",
                );
            }
        }
    }

    fn check_process_launched(&mut self, line: &str) {
        lazy_static! {
            static ref RE_PROCESS_LAUNCHED: Regex =
                Regex::new(r#"Process (\d+) launched: '.*' \(.*\)$"#).unwrap();
        }

        for cap in RE_PROCESS_LAUNCHED.captures_iter(line) {
            let process_pid = cap[1].to_string();

            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Process {} launched", process_pid),
            );

            self.process_pid = Some(process_pid);
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
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Process {} exited with exit code {}", pid, exit_code),
            );

            self.process_pid = None;
        }
    }

    fn check_process_and_thread_running(&mut self, line: &str) {
        lazy_static! {
            static ref RE_NOT_RUNNING: Regex =
                Regex::new(r#"^error: invalid (process|thread)$"#).unwrap();
        }

        for _ in RE_NOT_RUNNING.captures_iter(line) {
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::WARN,
                "No process running",
            );
        }
    }
}
