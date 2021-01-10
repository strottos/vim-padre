//! Python process handler
//!
//! This module performs the basic setup of and interfacing with PDB. It will
//! analyse the output of the text and work out what is happening then.

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use padre_core::debugger::{FileLocation, Variable};
use padre_core::server::{LogLevel, Notification, PadreError, PadreErrorKind};
#[cfg(not(test))]
use padre_core::util::{file_exists, get_file_full_path};
use padre_core::util::{jump_to_position, log_msg};
use padre_core::Result;

use bytes::{Bytes, BytesMut};
use futures::prelude::*;
use regex::Regex;
use tokio::io::{self as tokio_io, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::{BytesCodec, FramedRead};

/// Messages that can be sent to PDB for processing
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Launching,
    Restart,
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

/// Current status of PDB
#[derive(Debug, Clone, PartialEq)]
pub enum PDBStatus {
    Listening,
    Processing(Message),
}

/// Work out the arguments to send to python based on the python command given and the
/// run command specified
fn get_python_args<'a>(debugger_cmd: &str, run_cmd: Vec<&'a str>) -> Result<Vec<&'a str>> {
    let mut python_args = vec![];
    let mut script_args = vec![];

    // Now check the debugger and program to debug exist, if not error
    #[cfg(not(test))]
    {
        // Try getting the full path if the debugger doesn't exist
        if !file_exists(&debugger_cmd) {
            let debugger_cmd = get_file_full_path(&debugger_cmd);

            if !file_exists(&debugger_cmd) {
                return Err(PadreError::new(
                    PadreErrorKind::DebuggerError,
                    "Can't spawn debugger".to_string(),
                    format!("Can't spawn debugger as {} does not exist", debugger_cmd),
                ));
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

    Ok(python_args)
}

/// Main handler for spawning the PDB process
///
/// The status contains the process id of any PID running (or None if there isn't one) and the
#[derive(Debug)]
pub struct PythonProcess {
    process: Child,
    stdin_tx: mpsc::Sender<Bytes>,
    analyser: Arc<Mutex<PDBAnalyser>>,
    notifier_tx: mpsc::Sender<Notification>,
}

impl PythonProcess {
    /// Create and setup PDB
    ///
    /// Includes spawning the PDB process and all the relevant stdio handlers. In particular:
    /// - Sets up a `ReadOutput` from `util.rs` in order to read output from PDB;
    /// - Sets up a thread to read stdin and forward it onto PDB stdin;
    /// - Checks that PDB and the program to be ran both exist, otherwise panics.
    pub fn new(
        debugger_cmd: String,
        run_cmd: Vec<String>,
        notifier_tx: mpsc::Sender<Notification>,
    ) -> Self {
        let args =
            match get_python_args(&debugger_cmd[..], run_cmd.iter().map(|x| &x[..]).collect()) {
                Ok(args) => args,
                Err(err) => panic!("Can't spawn PDB, unknown args: {:?}", err),
            };

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

        let analyser = Arc::new(Mutex::new(PDBAnalyser::new(
            notifier_tx.clone(),
            format!("{}", process.id().unwrap()),
        )));

        // NB: Don't need stderr as it's taken from a process spawned with ptywrapper.py that will
        // wrap stderr into stdout.
        PythonProcess::setup_stdout(
            process
                .stdout
                .take()
                .expect("PDB process did not have a handle to stdout"),
            analyser.clone(),
        );
        let stdin_tx = PythonProcess::setup_stdin(
            process
                .stdin
                .take()
                .expect("Python process did not have a handle to stdin"),
            analyser.clone(),
        );

        PythonProcess {
            process,
            stdin_tx,
            analyser,
            notifier_tx,
        }
    }

    pub async fn stop(&mut self) {
        self.process.kill().await.unwrap();
    }

    /// Send a message to write to stdin
    pub fn send_msg(
        &mut self,
        message: Message,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let stdin_tx = self.stdin_tx.clone();
        let analyser = self.analyser.clone();

        let send_msg = match &message {
            Message::Launching => unreachable!(),
            Message::Restart => Bytes::from("run\n"),
            Message::Breakpoint(fl) => {
                Bytes::from(format!("break {}:{}\n", fl.name(), fl.line_num()))
            }
            Message::Unbreakpoint(fl) => {
                Bytes::from(format!("clear {}:{}\n", fl.name(), fl.line_num()))
            }
            Message::StepIn(count) => return self.step(Message::StepIn(*count), tx_done),
            Message::StepOver(count) => return self.step(Message::StepOver(*count), tx_done),
            Message::Continue => Bytes::from("continue\n"),
            Message::PrintVariable(v) => {
                return self.send_print_variable_message(v.clone(), tx_done)
            }
            Message::Custom => todo!(),
        };

        tokio::spawn(async move {
            analyser
                .lock()
                .unwrap()
                .analyse_message(message.clone(), tx_done);

            stdin_tx.send(send_msg).map(move |_| {}).await;
        });
    }

    fn step(&self, message: Message, tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>) {
        let stdin_tx = self.stdin_tx.clone();
        let analyser = self.analyser.clone();

        let (bytes, count) = match message {
            Message::StepIn(count) => (Bytes::from("step\n"), count),
            Message::StepOver(count) => (Bytes::from("next\n"), count),
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
                    .analyse_message(message.clone(), Some(tx));

                stdin_tx.send(bytes.clone()).map(move |_| {}).await;

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

    fn send_print_variable_message(
        &mut self,
        v: Variable,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        let analyser = self.analyser.clone();
        let stdin_tx = self.stdin_tx.clone();

        let variable_name = v.name().to_string();

        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();

            analyser.lock().unwrap().analyse_message(
                Message::PrintVariable(Variable::new(variable_name.clone())),
                Some(tx),
            );

            stdin_tx
                .send(Bytes::from(format!("print({})\n", v.name())))
                .map(move |_| {})
                .await;

            // TODO: Errors
            let value = rx.await.unwrap();

            let (tx, rx) = oneshot::channel();

            analyser.lock().unwrap().analyse_message(
                Message::PrintVariable(Variable::new(format!("type({})", variable_name))),
                Some(tx),
            );

            stdin_tx
                .send(Bytes::from(format!("print(type({}))\n", v.name())))
                .map(move |_| {})
                .await;

            // TODO: Errors
            let type_ = rx.await.unwrap();

            tx_done
                .unwrap()
                .send(Ok(serde_json::json!({
                    "variable": variable_name,
                    "type": type_.unwrap().get("value").unwrap(),
                    "value": value.unwrap().get("value").unwrap(),
                })))
                .unwrap();
        });
    }

    /// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
    /// stdin of a process.
    fn setup_stdin(
        mut child_stdin: ChildStdin,
        analyser: Arc<Mutex<PDBAnalyser>>,
    ) -> mpsc::Sender<Bytes> {
        let (stdin_tx, mut stdin_rx) = mpsc::channel(32);
        let tx = stdin_tx.clone();

        tokio::spawn(async move {
            let tokio_stdin = tokio_io::stdin();
            let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
            while let Some(line) = reader.next().await {
                analyser
                    .lock()
                    .unwrap()
                    .set_status(PDBStatus::Processing(Message::Custom));
                let buf = line.unwrap().freeze();
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
            }
        });

        stdin_tx
    }

    /// Perform setup of reading PDB stdout, analysing it and writing it back to stdout.
    fn setup_stdout(stdout: ChildStdout, analyser: Arc<Mutex<PDBAnalyser>>) {
        tokio::spawn(async move {
            let mut reader = FramedRead::new(stdout, BytesCodec::new());
            while let Some(Ok(text)) = reader.next().await {
                analyser.lock().unwrap().handle_output(text);
            }
        });
    }
}

#[derive(Debug)]
pub struct PDBAnalyser {
    pdb_status: PDBStatus,
    process_pid: String,
    awakener: Option<oneshot::Sender<Result<serde_json::Value>>>,
    notifier_tx: mpsc::Sender<Notification>,
    print_variable_value: String,
    reporting_location: bool,
}

impl PDBAnalyser {
    pub fn new(notifier_tx: mpsc::Sender<Notification>, process_pid: String) -> Self {
        PDBAnalyser {
            pdb_status: PDBStatus::Processing(Message::Launching),
            process_pid,
            // Gets set when trying to listen for when it's ready to process commands
            awakener: None,
            notifier_tx,
            print_variable_value: "".to_string(),
            reporting_location: true,
        }
    }

    pub fn handle_output(&mut self, bytes: BytesMut) {
        let text = String::from_utf8_lossy(&bytes[..]).to_string();
        print!("{}", text);
        io::stdout().flush().unwrap();

        match &self.pdb_status {
            PDBStatus::Processing(msg) => match msg {
                Message::PrintVariable(var) => {
                    let text = self.print_variable_value.clone()
                        + &self.strip_gibberish(&String::from_utf8_lossy(&bytes[..]).to_string());

                    let mut from = 0;
                    let mut to = text.len();

                    let prefix = format!("print({})\r\n", var.name());
                    let prefix_length = prefix.len();
                    if to >= prefix_length && &text[0..prefix_length] == prefix {
                        from += prefix_length;
                    }

                    let suffix = "\r\n(Pdb) ";
                    let suffix_length = suffix.len();
                    if to >= suffix_length && &text[to - suffix_length..to] == suffix {
                        to -= suffix_length;
                    }

                    self.print_variable_value = text[from..to].to_string();
                }
                _ => {}
            },
            _ => {}
        }

        for line in text.split("\r\n") {
            match &self.pdb_status {
                PDBStatus::Listening => {}
                PDBStatus::Processing(msg) => match msg {
                    Message::Restart | Message::Launching => {
                        self.check_location(&line[..]);
                    }
                    Message::Breakpoint(_) => {
                        self.check_breakpoint(&line[..]);
                    }
                    Message::StepIn(_) | Message::StepOver(_) | Message::Continue => {
                        self.check_location(&line[..]);
                        self.check_returning(&line[..]);
                        self.check_exited(&line[..]);
                    }
                    Message::Custom => {
                        self.check_breakpoint(&line[..]);
                        self.check_location(&line[..]);
                        self.check_returning(&line[..]);
                        self.check_exited(&line[..]);
                    }
                    _ => {}
                },
            };
        }

        match text.split("\r\n").last() {
            Some(l) => {
                if str::ends_with(l, "(Pdb) ") {
                    match &self.pdb_status {
                        PDBStatus::Processing(msg) => match msg {
                            Message::PrintVariable(var) => {
                                let tx_option = self.awakener.take();
                                match tx_option {
                                    Some(tx) => {
                                        // TODO: Error checks for proper thing
                                        if self.print_variable_value
                                            == format!(
                                                "error: no variable named '{}' found in this frame",
                                                var.name()
                                            )
                                            || self.print_variable_value == "error: invalid process"
                                        {
                                            tx.send(Err(PadreError::new(
                                                PadreErrorKind::DebuggerError,
                                                "Variable not found".to_string(),
                                                format!("Variable '{}' not found", var.name()),
                                            )))
                                            .unwrap()
                                        } else {
                                            tx.send(Ok(serde_json::json!({
                                                "value": self.print_variable_value,
                                            })))
                                            .unwrap();
                                            self.print_variable_value = "".to_string();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    };
                    self.set_status(PDBStatus::Listening);
                }
            }
            None => {}
        }
    }

    /// We get some kind of terminal this weird pattern out of Python 3.9 on
    /// a mac sometimes:
    /// <class 'int'>\r\n\x1b[?2004h(Pdb)
    /// Patterns we strip at present:
    /// \x1b[?2004h
    /// \x1b[?2004l\r
    /// Stripping it out for now, not ideal but will have to do for now.
    fn strip_gibberish(&self, text: &str) -> String {
        lazy_static! {
            static ref RE_GIBBERISH: Regex = Regex::new(r#"(.*)\x1b\[\?2004[a-z]\r?(.*)"#).unwrap();
        }

        let mut ret1 = "".to_string();

        let split1: Vec<&str> = text.split("\x1b[?2004h").collect();
        for s in split1 {
            ret1 += s
        }

        let mut ret2 = "".to_string();

        let split2: Vec<&str> = ret1.split("\x1b[?2004l\r").collect();
        for s in split2 {
            ret2 += s
        }

        ret2
    }

    pub fn set_status(&mut self, pdb_status: PDBStatus) {
        match &self.pdb_status {
            PDBStatus::Listening => {}
            _ => {
                match pdb_status {
                    PDBStatus::Listening => {
                        let tx_option = self.awakener.take();
                        match tx_option {
                            Some(tx) => {
                                tx.send(Ok(serde_json::json!({}))).unwrap();
                            }
                            _ => {}
                        };
                    }
                    _ => {}
                };
            }
        }
        self.pdb_status = pdb_status;
    }

    /// Sets up the analyser ready for analysing the message.
    ///
    /// It sets the status of the analyser to Processing for that message and if given
    /// it marks the analyser to send a message to `tx_done` to indicate when the
    /// message is processed.
    pub fn analyse_message(
        &mut self,
        msg: Message,
        tx_done: Option<oneshot::Sender<Result<serde_json::Value>>>,
    ) {
        self.pdb_status = PDBStatus::Processing(msg);
        self.awakener = tx_done;
    }

    fn check_breakpoint(&self, line: &str) {
        lazy_static! {
            static ref RE_BREAKPOINT: Regex =
                Regex::new("Breakpoint (\\d*) at (.*):(\\d*)$").unwrap();
        }

        for l in line.split("\r") {
            for cap in RE_BREAKPOINT.captures_iter(l) {
                let file = cap[2].to_string();
                let line = cap[3].parse::<u64>().unwrap();
                log_msg(
                    self.notifier_tx.clone(),
                    LogLevel::INFO,
                    &format!("Breakpoint set at file {} and line number {}", file, line),
                );
            }
        }
    }

    fn check_location(&self, line: &str) {
        if !self.reporting_location {
            return;
        }

        lazy_static! {
            static ref RE_JUMP_TO_POSITION: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)$").unwrap();
        }

        for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
            let file = cap[1].to_string();
            let line = cap[2].parse::<u64>().unwrap();
            jump_to_position(self.notifier_tx.clone(), &file, line);
        }
    }

    fn check_returning(&self, line: &str) {
        if !self.reporting_location {
            return;
        }

        lazy_static! {
            static ref RE_RETURNING: Regex =
                Regex::new("^> (.*)\\((\\d*)\\)[<>\\w]*\\(\\)->(.*)$").unwrap();
        }

        for cap in RE_RETURNING.captures_iter(line) {
            let file = cap[1].to_string();
            let line = cap[2].parse::<u64>().unwrap();
            let return_value = cap[3].to_string();
            jump_to_position(self.notifier_tx.clone(), &file, line);
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!("Returning value {}", return_value),
            );
        }
    }

    fn check_exited(&mut self, line: &str) {
        lazy_static! {
            static ref RE_PROCESS_EXITED: Regex =
                Regex::new("^The program finished and will be restarted$").unwrap();
            static ref RE_PROCESS_EXITED_WITH_CODE: Regex =
                Regex::new("^The program exited via sys.exit\\(\\)\\. Exit status: (-?\\d*)$")
                    .unwrap();
        }

        let exited = |exit_code| {
            log_msg(
                self.notifier_tx.clone(),
                LogLevel::INFO,
                &format!(
                    "Process {} exited with exit code {}",
                    self.process_pid, exit_code
                ),
            );
        };

        for _ in RE_PROCESS_EXITED.captures_iter(line) {
            &exited(0);
        }

        for cap in RE_PROCESS_EXITED_WITH_CODE.captures_iter(line) {
            let exit_code = cap[1].parse::<i64>().unwrap();
            &exited(exit_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_get_args_basic_command() {
        let args = get_python_args("/usr/bin/python3", vec!["test.py", "arg1"]).unwrap();
        assert_eq!(args, vec!["-m", "pdb", "--", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_recognises_matching_python_command() {
        let args = get_python_args("/usr/bin/python3", vec!["python3", "test.py", "arg1"]).unwrap();
        assert_eq!(args, vec!["-m", "pdb", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_leaves_non_matching_python_command() {
        let args =
            get_python_args("/usr/bin/python3.7", vec!["python3", "test.py", "arg1"]).unwrap();
        assert_eq!(args, vec!["-m", "pdb", "--", "python3", "test.py", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_module_running() {
        let args = get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-m", "abc", "--", "arg1"],
        )
        .unwrap();
        assert_eq!(args, vec!["-m", "pdb", "-m", "abc", "--", "arg1"]);
    }

    #[test]
    fn check_get_args_accepts_command_arguments() {
        let args = get_python_args(
            "/usr/bin/python3",
            vec!["python3", "-c", "print('Hello, World!')"],
        )
        .unwrap();
        assert_eq!(args, vec!["-m", "pdb", "-c", "print('Hello, World!')"]);
    }

    #[tokio::test]
    async fn analyser_startup() {
        let (tx, _) = mpsc::channel(1);
        let mut analyser = PDBAnalyser::new(tx, "12345".to_string());
        assert_eq!(
            analyser.pdb_status,
            PDBStatus::Processing(Message::Launching)
        );
        analyser.handle_output(BytesMut::from(
            "> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n",
        ));
        assert_eq!(
            analyser.pdb_status,
            PDBStatus::Processing(Message::Launching)
        );
        analyser.handle_output(BytesMut::from("(Pdb) "));
        assert_eq!(analyser.pdb_status, PDBStatus::Listening);
    }

    #[tokio::test]
    async fn analyser_custom_syntax_error() {
        let (tx, _) = mpsc::channel(1);
        let mut analyser = PDBAnalyser::new(tx, "12345".to_string());
        analyser.handle_output(BytesMut::from(
            "> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ",
        ));
        analyser.set_status(PDBStatus::Processing(Message::Custom));
        analyser.handle_output(BytesMut::from("do something nonsensical\r\n"));
        assert_eq!(analyser.pdb_status, PDBStatus::Processing(Message::Custom));
        analyser.handle_output(BytesMut::from("*** SyntaxError: invalid syntax\r\n(Pdb) "));
        assert_eq!(analyser.pdb_status, PDBStatus::Listening);
    }

    #[tokio::test]
    async fn analyser_padre_message_wakeup() {
        let (tx, _) = mpsc::channel(1);
        let mut analyser = PDBAnalyser::new(tx, "12345".to_string());
        analyser.handle_output(BytesMut::from(
            "> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ",
        ));
        let msg = Message::Breakpoint(FileLocation::new("test.py".to_string(), 2));
        analyser.set_status(PDBStatus::Processing(msg.clone()));
        assert_eq!(analyser.pdb_status, PDBStatus::Processing(msg));
        analyser.handle_output(BytesMut::from("Breakpoint 1 at test.py:2\r\n(Pdb) "));
        assert_eq!(analyser.pdb_status, PDBStatus::Listening);
    }

    #[tokio::test]
    async fn analyser_print_message() {
        let (tx, _) = mpsc::channel(1);
        let mut analyser = PDBAnalyser::new(tx, "12345".to_string());
        analyser.handle_output(BytesMut::from(
            "> /Users/me/test.py(1)<module>()\r\n-> abc = 123\r\n(Pdb) ",
        ));
        let msg = Message::PrintVariable(Variable::new("abc".to_string()));
        analyser.set_status(PDBStatus::Processing(msg.clone()));
        analyser.handle_output(BytesMut::from("print(abc)\r\n"));
        analyser.handle_output(BytesMut::from("123\r\n"));
        assert_eq!(analyser.print_variable_value, "123\r\n".to_string());
        analyser.handle_output(BytesMut::from("(Pdb) "));
    }
}
