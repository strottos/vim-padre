//! lldb client debugger

use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};

use crate::debugger::tty_process::spawn_process;
use crate::debugger::Debugger;
use crate::notifier::{LogLevel, Notifier};

use bytes::Bytes;
use nix::fcntl::OFlag;
use nix::pty::{grantpt, posix_openpt, unlockpt};
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_cmd: String,
    run_cmd: Vec<String>,
    lldb_in_tx: Option<Sender<Bytes>>,
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
            lldb_in_tx: None,
        }
    }

    fn send_lldb(&mut self, bytes: Bytes) {
        assert!(!self.lldb_in_tx.is_none());
        let lldb_in_tx = self.lldb_in_tx.clone().unwrap();
        tokio::spawn(
            lldb_in_tx
                .send(bytes)
                .map(|_| {})
                .map_err(|e| println!("Error sending to LLDB: {}", e)),
        );
    }
}

impl Debugger for ImplDebugger {
    fn setup(&mut self) {
        if !Path::new(&self.run_cmd[0]).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]),
            );
            println!("Can't spawn LLDB as {} does not exist", self.run_cmd[0]);
            exit(1);
        }

        let (lldb_in_tx, lldb_in_rx) = mpsc::channel(1);
        let (lldb_out_tx, lldb_out_rx) = mpsc::channel(32);

        self.lldb_in_tx = Some(lldb_in_tx);

        let mut cmd = vec![self.debugger_cmd.clone(), "--".to_string()];
        cmd.extend(self.run_cmd.clone());
        spawn_process(cmd, lldb_in_rx, lldb_out_tx);

        self.send_lldb(Bytes::from(&b"settings set stop-line-count-after 0\n"[..]));
        self.send_lldb(Bytes::from(&b"settings set stop-line-count-before 0\n"[..]));
        self.send_lldb(Bytes::from(&b"settings set frame-format frame #${frame.index}{ at ${line.file.fullpath}:${line.number}}\\n\n"[..]));

        self.notifier.lock().unwrap().signal_started();
    }
}

impl Future for ImplDebugger {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        Ok(Async::NotReady)
    }
}
