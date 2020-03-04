//! Node process handler
//!
//! This module performs the basic setup and spawning of the Node process.

use padre_core::util::{check_and_spawn_process, read_output, setup_stdin};

use futures::prelude::*;
use regex::Regex;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::sync::mpsc::Sender;

/// Main handler for spawning the Node process
#[derive(Debug)]
pub struct Process {
    process: Child,
}

impl Process {
    /// Create a new Process
    ///
    /// Run Node program, including handling forwarding stdin onto the Node interpreter but
    /// not used to analyse the program as some of the other debuggers are.
    pub fn new(debugger_cmd: String, run_cmd: Vec<String>, tx: Sender<String>) -> Self {
        let mut process =
            check_and_spawn_process(vec![debugger_cmd, "--inspect-brk=0".to_string()], run_cmd);

        setup_stdin(
            process
                .stdin
                .take()
                .expect("Python process did not have a handle to stdin"),
            false,
        );

        let stdout = process
            .stdout
            .take()
            .expect("Python process did not have a handle to stdout");

        // Perform setup of reading Node stdout and writing it back to PADRE stdout.
        //
        // Also checks for the line about where the Debugger is listening as this is
        // required for the websocket setup. This comes through stdout as it all routes
        // through stdout due to the pty wrapper.
        tokio::spawn(async move {
            lazy_static! {
                static ref RE_NODE_STARTED: Regex =
                    Regex::new("^Debugger listening on (ws://127.0.0.1:\\d+/.*)$").unwrap();
            }

            let mut node_setup = false;

            let mut reader = read_output(BufReader::new(stdout));

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
                    print!("{}", text);
                }
            }
        });

        Process { process }
    }

    pub fn stop(&mut self) {
        self.process.kill().unwrap();
    }

    pub fn get_pid(&self) -> u64 {
        self.process.id() as u64
    }
}
