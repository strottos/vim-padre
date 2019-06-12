//! notifier functions and traits

// The notifier is responsible for communicating with everything connected to PADRE

use std::net::SocketAddr;

use crate::server::PadreResponse;

use tokio::prelude::*;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum LogLevel {
    CRITICAL = 1,
    ERROR,
    WARN,
    INFO,
    DEBUG,
}

#[derive(Debug)]
struct Listener {
    sender: Sender<PadreResponse>,
    addr: SocketAddr,
    has_started: bool,
}

#[derive(Debug)]
pub struct Notifier {
    listeners: Vec<Listener>,
}

impl Notifier {
    pub fn new() -> Notifier {
        Notifier {
            listeners: Vec::new(),
        }
    }

    pub fn add_listener(&mut self, sender: Sender<PadreResponse>, addr: SocketAddr) {
        self.listeners.push(Listener {
            sender,
            addr,
            has_started: false,
        });
    }

    pub fn remove_listener(&mut self, addr: &SocketAddr) {
        self.listeners.retain(|listener| listener.addr != *addr);
    }

    pub fn signal_started(&mut self) {
        let msg = PadreResponse::Notify("padre#debugger#SignalPADREStarted".to_string(), vec![]);
        for mut listener in self.listeners.iter_mut() {
            if !listener.has_started {
                let sender = listener.sender.clone();
                tokio::spawn(
                    sender
                        .send(msg.clone())
                        .map(|_| ())
                        .map_err(|e| eprintln!("Notifier can't send to socket: {}", e)),
                );
                listener.has_started = true;
            }
        }
    }

    pub fn signal_exited(&mut self, pid: u64, exit_code: u64) {
        let msg = PadreResponse::Notify(
            "padre#debugger#ProcessExited".to_string(),
            vec![serde_json::json!(exit_code), serde_json::json!(pid)],
        );
        self.send_msg(msg);
    }

    pub fn log_msg(&mut self, level: LogLevel, msg: String) {
        let msg = PadreResponse::Notify(
            "padre#debugger#Log".to_string(),
            vec![serde_json::json!(level as u8), serde_json::json!(msg)],
        );
        self.send_msg(msg);
    }

    pub fn jump_to_position(&mut self, file: String, line: u64) {
        let msg = PadreResponse::Notify(
            "padre#debugger#JumpToPosition".to_string(),
            vec![serde_json::json!(file), serde_json::json!(line)],
        );
        self.send_msg(msg);
    }

    pub fn breakpoint_set(&mut self, file: String, line: u64) {
        let msg = PadreResponse::Notify(
            "padre#debugger#BreakpointSet".to_string(),
            vec![serde_json::json!(file), serde_json::json!(line)],
        );
        self.send_msg(msg);
    }

    fn send_msg(&mut self, msg: PadreResponse) {
        for listener in self.listeners.iter_mut() {
            let sender = listener.sender.clone();
            tokio::spawn(
                sender
                    .send(msg.clone())
                    .map(|_| ())
                    .map_err(|e| eprintln!("Notifier can't send to socket: {}", e)),
            );
        }
    }
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
