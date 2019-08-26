//! Node process handler
//!
//! This module performs the basic setup and spawning of the Node process.

use std::io::BufReader;

use crate::util::{check_and_spawn_process, read_output, setup_stdin};

use regex::Regex;
use tokio::prelude::*;
use tokio::sync::mpsc::Sender;
use tokio_process::{Child, ChildStderr, ChildStdout};

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
        tokio::spawn(
            read_output(BufReader::new(stdout))
                .for_each(move |text| {
                    print!("{}", text);
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading Node stdout: {}", e)),
        );
    }

    /// Perform setup of reading Node stderr and writing it back to PADRE stderr.
    ///
    /// Also checks for the line about where the Debugger is listening as this is
    /// required for the websocket setup.
    fn setup_stderr(&mut self, stderr: ChildStderr, tx: Sender<String>) {
        lazy_static! {
            static ref RE_NODE_STARTED: Regex =
                Regex::new("^Debugger listening on (ws://127.0.0.1:\\d+/.*)$").unwrap();
        }

        let mut node_setup = false;

        tokio::spawn(
            read_output(BufReader::new(stderr))
                .for_each(move |text| {
                    if !node_setup {
                        'node_setup_start: for line in text.split("\n") {
                            for cap in RE_NODE_STARTED.captures_iter(&line) {
                                tx.clone().send(cap[1].to_string()).wait().unwrap();
                                node_setup = true;
                                break 'node_setup_start;
                            }
                        }
                    } else {
                        eprint!("{}", text);
                    }
                    Ok(())
                })
                .map_err(|e| eprintln!("Err reading Node stderr: {}", e)),
        );
    }
}
