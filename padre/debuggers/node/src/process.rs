//! Node process handler
//!
//! This module performs the basic setup and spawning of the Node process.

use padre_core::util::{check_and_spawn_process, read_output, setup_stdin};

use futures::prelude::*;
use regex::Regex;
use tokio::io::BufReader;
use tokio::prelude::*;
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::sync::mpsc::Sender;

/// Main handler for spawning the Node process
#[derive(Debug)]
pub struct Process {
    debugger_cmd: Option<String>,
    run_cmd: Option<Vec<String>>,
    process: Option<Child>,
}

impl Process {
    /// Create a new Process
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>) -> Self {
        Process {
            debugger_cmd: Some(debugger_cmd),
            run_cmd: Some(run_cmd),
            process: None,
        }
    }

    /// Run Node program, including handling forwarding stdin onto the Node interpreter but
    /// not used to analyse the program as some of the other debuggers are.
    pub fn run(&mut self, tx: Sender<String>) {
        let mut process = check_and_spawn_process(
            vec![
                self.debugger_cmd.take().unwrap(),
                "--inspect-brk=0".to_string(),
            ],
            self.run_cmd.take().unwrap(),
        );

        setup_stdin(
            process
                .stdin()
                .take()
                .expect("Python process did not have a handle to stdin"),
            false,
        );

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
            tx,
        );

        self.process = Some(process);
    }

    pub fn get_pid(&self) -> u64 {
        self.process.as_ref().unwrap().id() as u64
    }

    /// Perform setup of reading Node stdout and writing it back to PADRE stdout.
    fn setup_stdout(&mut self, stdout: ChildStdout) {
        tokio::spawn(async move {
            let mut reader = read_output(BufReader::new(stdout));
            while let Some(Ok(text)) = reader.next().await {
                print!("{}", text);
            }
        });
    }

    /// Perform setup of reading Node stderr and writing it back to PADRE stderr.
    ///
    /// Also checks for the line about where the Debugger is listening as this is
    /// required for the websocket setup.
    fn setup_stderr(&mut self, stderr: ChildStderr, tx: Sender<String>) {
        tokio::spawn(async move {
            lazy_static! {
                static ref RE_NODE_STARTED: Regex =
                    Regex::new("^Debugger listening on (ws://127.0.0.1:\\d+/.*)$").unwrap();
            }

            let mut node_setup = false;

            let mut reader = read_output(BufReader::new(stderr));

            while let Some(Ok(text)) = reader.next().await {
                if !node_setup {
                    'node_setup_start: for line in text.split("\n") {
                        let mut uri = None;
                        for cap in RE_NODE_STARTED.captures_iter(&line) {
                            uri = Some(cap[1].to_string().clone());
                            node_setup = true;
                        }
                        match uri {
                            Some(uri) => {
                                tx.clone().send(uri).await.unwrap();
                                break 'node_setup_start;
                            }
                            None => {}
                        };
                    }
                } else {
                    eprint!("{}", text);
                }
            }
        });
    }
}
