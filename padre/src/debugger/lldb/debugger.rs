//! lldb client debugger

use std::sync::{Arc, Mutex};

use crate::notifier::Notifier;
use crate::debugger::Debugger;

use tokio::prelude::*;

#[derive(Debug)]
pub struct ImplDebugger {
    notifier: Arc<Mutex<Notifier>>,
    run_cmd: Vec<String>,
}

impl ImplDebugger {
    pub fn new(notifier: Arc<Mutex<Notifier>>, run_cmd: Vec<String>) -> ImplDebugger {
        ImplDebugger { notifier, run_cmd }
    }
}

impl Debugger for ImplDebugger {
    fn setup(self) {
    }
}

impl Future for ImplDebugger {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        Ok(Async::NotReady)
    }
}
