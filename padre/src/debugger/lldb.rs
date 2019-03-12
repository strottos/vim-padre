//! lldb client debugger

use std::sync::{Arc, Mutex};

use crate::debugger::Debugger;
use crate::notifier::Notifier;

pub struct LLDB {
    notifier: Arc<Mutex<Notifier>>,
}

impl Debugger for LLDB {
}

impl LLDB {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> LLDB {
        LLDB {
            notifier: notifier,
        }
    }
}
