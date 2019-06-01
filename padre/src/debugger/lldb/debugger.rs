//! lldb client debugger

use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};
use crate::debugger::Debugger;
use crate::debugger::tty_process::spawn_process;

use nix::fcntl::OFlag;
use nix::pty::{grantpt, posix_openpt, unlockpt};
use tokio::prelude::*;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
}

impl ImplDebugger {
    pub fn new(notifier: Arc<Mutex<Notifier>>, debugger_cmd: String, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger { notifier, debugger_cmd, run_cmd }
    }
}

impl Debugger for ImplDebugger {
    fn setup(&self) {
        if !Path::new(&self.run_cmd[0]).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]),
            );
            println!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]);
            exit(1);
        }

        let (tx, rx) = mpsc::channel(32);

        let mut cmd = vec!(self.debugger_cmd.clone(), "--".to_string());
        cmd.extend(self.run_cmd.clone());
        spawn_process(cmd, rx);
    }
}

impl Future for ImplDebugger {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        Ok(Async::NotReady)
    }
}
