//! Node debugger

use std::io;
use std::process::{exit, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::debugger::Debugger;
use crate::notifier::Notifier;
use crate::util;

use hyper::Client;
use tokio::prelude::*;
use tokio_process::CommandExt;

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    node_process: Option<Command>,
}

impl ImplDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_cmd: String,
        run_cmd: Vec<String>,
    ) -> ImplDebugger {
        ImplDebugger {
            notifier,
            debugger_cmd,
            run_cmd,
            node_process: None,
        }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {}

    fn teardown(&mut self) {
        exit(0);
    }

    fn has_started(&self) -> bool {
        true
    }

    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let port = util::get_unused_localhost_port();
        let mut cmd = Command::new(self.debugger_cmd.clone())
            .arg(format!("--inspect-brk={}", port))
            .arg("--")
            .args(self.run_cmd.clone())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_async()
            .expect("Can't spawn node");

        let mut node_stdin = cmd.stdin().take().unwrap();
        let node_stderr = cmd.stderr().take().unwrap();

        thread::spawn(move || {
            let mut stdin = io::stdin();
            loop {
                let mut buf = vec![0; 1024];
                let n = match stdin.read(&mut buf) {
                    Err(_) | Ok(0) => break,
                    Ok(n) => n,
                };
                buf.truncate(n);
                node_stdin.write(&buf).unwrap();
            }
        });

        let reader = io::BufReader::new(node_stderr);
        let lines = tokio::io::lines(reader);
        tokio::spawn(
            lines.for_each(move |l| {
                eprintln!("{}", l);
                Ok(())
            })
            .map(|a| println!("stderr: {:?}", a))
            .map_err(|e| println!("stderr err: {:?}", e))
        );

        tokio::spawn(
            cmd.map(|a| {
                println!("process: {}", a);
            })
            .map_err(|e| {
                eprintln!("Error spawning node: {}", e);
            }),
        );

        let uri = format!("http://127.0.0.1:{}/json", port).parse().unwrap();

        tokio::spawn(
            Client::new()
                .get(uri)
                .map(|out| {
                    println!("Out: {:?}", out);
                })
                .map_err(|e| {
                    println!("Error sending to node: {}", e);
                })
        );
        //let node_details: Vec<NodeDetails> = response.json().unwrap();

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn breakpoint(
        &mut self,
        _file: String,
        _line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }

    fn print(
        &mut self,
        _variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"OK"});
            Ok(resp)
        });

        Box::new(f)
    }
}

#[derive(Deserialize, Debug)]
struct NodeDetails {
    description: String,
    devtoolsFrontendUrl: String,
    faviconUrl: String,
    id: String,
    title: String,
    url: String,
    webSocketDebuggerUrl: String,
}

#[cfg(test)]
mod tests {}
