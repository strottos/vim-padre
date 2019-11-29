//! Notifier
//!
//! This module contains tools for notifying every socket connection about an
//! event.

use std::net::SocketAddr;
use std::sync::Mutex;

use crate::server::{Notification, PadreSend};

use tokio::sync::mpsc::Sender;

lazy_static! {
    static ref NOTIFIER: Mutex<Notifier> = { Mutex::new(Notifier::new()) };
}

/// Log level to log at, clients can choose to filter messages at certain log
/// levels
///
/// TODO: Allow each socket to configure it's own log level
#[derive(Debug)]
pub enum LogLevel {
    CRITICAL = 1,
    ERROR,
    WARN,
    INFO,
    DEBUG,
}

/// A `Listener` is a wrapper around the socket that we can use to send
/// messages to
#[derive(Debug)]
struct Listener {
    sender: Sender<PadreSend>,
    addr: SocketAddr,
}

/// The `Notifier` creates the main singleton object for PADRE to communicate
/// with it's listeners.
///
/// We store a vector of `Listener`s and when one is finished with we drop it
/// from the list.
#[derive(Debug)]
struct Notifier {
    listeners: Vec<Listener>,
}

impl Notifier {
    /// Constructor for creating the Notifier object
    fn new() -> Notifier {
        Notifier {
            listeners: Vec::new(),
        }
    }

    /// Add a listener to the notifier
    ///
    /// Should be called when a new connection is added.
    fn add_listener(&mut self, sender: Sender<PadreSend>, addr: SocketAddr) {
        self.listeners.push(Listener { sender, addr });
    }

    /// Remove a listener from the notifier
    ///
    /// Should be called when a connection is dropped.
    fn remove_listener(&mut self, addr: &SocketAddr) {
        self.listeners.retain(|listener| listener.addr != *addr);
    }

    /// Send the message to all clients
    fn send_msg(&mut self, msg: Notification) {
        for listener in self.listeners.iter_mut() {
            let msg_copy = msg.clone();
            let mut sender = listener.sender.clone();
            tokio::spawn(async move {
                if let Err(e) = sender.send(PadreSend::Notification(msg_copy)).await {
                    eprintln!("Notifier can't send to socket: {}", e);
                }
            });
        }
    }
}

/// Add a listener to the notifier
///
/// Should be called when a new connection is added.
pub fn add_listener(sender: Sender<PadreSend>, addr: SocketAddr) {
    NOTIFIER.lock().unwrap().add_listener(sender, addr);
}

/// Remove a listener from the notifier
///
/// Should be called when a connection is dropped.
pub fn remove_listener(addr: &SocketAddr) {
    NOTIFIER.lock().unwrap().remove_listener(addr);
}

/// Notify that a process has exited
pub fn signal_exited(pid: u64, exit_code: i64) {
    let msg = Notification::new(
        "padre#debugger#ProcessExited".to_string(),
        vec![serde_json::json!(exit_code), serde_json::json!(pid)],
    );
    NOTIFIER.lock().unwrap().send_msg(msg);
}

/// Send a log message
pub fn log_msg(level: LogLevel, msg: &str) {
    let msg = Notification::new(
        "padre#debugger#Log".to_string(),
        vec![serde_json::json!(level as u8), serde_json::json!(msg)],
    );
    NOTIFIER.lock().unwrap().send_msg(msg);
}

/// Notify about a code position change
pub fn jump_to_position(file: &str, line: u64) {
    let msg = Notification::new(
        "padre#debugger#JumpToPosition".to_string(),
        vec![serde_json::json!(file), serde_json::json!(line)],
    );
    NOTIFIER.lock().unwrap().send_msg(msg);
}

/// Notify that a breakpoint has been set
pub fn breakpoint_set(file: &str, line: u64) {
    let msg = Notification::new(
        "padre#debugger#BreakpointSet".to_string(),
        vec![serde_json::json!(file), serde_json::json!(line)],
    );
    NOTIFIER.lock().unwrap().send_msg(msg);
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use tokio::sync::mpsc;

    fn create_notifier_with_listeners() -> super::Notifier {
        let mut notifier = super::Notifier::new();

        let (sender, _) = mpsc::channel(1);
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        notifier.add_listener(sender, socket_addr);

        let (sender, _) = mpsc::channel(1);
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

        notifier.add_listener(sender, socket_addr);

        notifier
    }

    #[test]
    fn check_can_add_listeners() {
        let notifier = create_notifier_with_listeners();

        assert_eq!(notifier.listeners.len(), 2);
    }

    #[test]
    fn check_can_remove_listener() {
        let mut notifier = create_notifier_with_listeners();

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        notifier.remove_listener(&socket_addr);

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        notifier.remove_listener(&socket_addr);

        assert_eq!(notifier.listeners.len(), 0);
    }
}
