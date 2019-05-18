//! notifier functions and traits

// The notifier is responsible for communicating with everything connected to PADRE

use std::io::Write;
use std::net::SocketAddr;

use tokio::io::WriteHalf;
use tokio::net::TcpStream;

pub enum LogLevel {
    CRITICAL = 1,
    ERROR,
    WARN,
    INFO,
    DEBUG,
}

#[derive(Debug)]
struct Listener {
    writer: WriteHalf<TcpStream>,
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

    pub fn add_listener(&mut self, writer: WriteHalf<TcpStream>, addr: SocketAddr) {
        println!("Adding listener: {:?}", addr);
        self.listeners.push(Listener {
            writer,
            addr,
            has_started: false,
        });
    }

    pub fn signal_started(&mut self) {
        let msg = format!("[\"call\",\"padre#debugger#SignalPADREStarted\",[]]");
        for mut listener in self.listeners.iter_mut() {
            if !listener.has_started {
                match listener.writer.write(msg.as_bytes()) {
                    Ok(_) => (),
                    Err(error) => {
                        println!("Can't send to socket: {}", error);
                    }
                }
                listener.has_started = true;
            }
        }
    }

    pub fn signal_exited(&mut self, pid: u32, exit_code: u8) {
        let msg = format!(
            "[\"call\",\"padre#debugger#ProcessExited\",[{},{}]]",
            exit_code, pid
        );
        self.send_msg(msg);
    }

    pub fn log_msg(&mut self, level: LogLevel, msg: String) {
        let msg = format!(
            "[\"call\",\"padre#debugger#Log\",[{},{}]]",
            level as i32,
            json::stringify(msg)
        );
        self.send_msg(msg);
    }

    pub fn jump_to_position(&mut self, file: String, line: u32) {
        let msg = format!(
            "[\"call\",\"padre#debugger#JumpToPosition\",[{},{}]]",
            json::stringify(file),
            line
        );
        self.send_msg(msg);
    }

    pub fn breakpoint_set(&mut self, file: String, line: u32) {
        let msg = format!(
            "[\"call\",\"padre#debugger#BreakpointSet\",[{},{}]]",
            json::stringify(file),
            line
        );
        self.send_msg(msg);
    }

    //    pub fn breakpoint_unset(&self, file: String, line: u32) {
    //        let msg = format!("[\"call\",\"padre#debugger#BreakpointUnset\",[{},{}]]",
    //                          json::stringify(file), line);
    //        self.send_msg(msg);
    //    }

    pub fn remove_listener(&mut self, addr: &SocketAddr) {
        println!("Listeners: {:?}", &self.listeners);
        self.listeners.retain(|listener| listener.addr != *addr);
        println!("Listeners: {:?}", &self.listeners);
    }

    fn send_msg(&mut self, msg: String) {
        for listener in self.listeners.iter_mut() {
            match listener.writer.write(msg.as_bytes()) {
                Ok(_) => (),
                Err(error) => {
                    println!("Notifier can't send to socket: {}", error);
                }
            }
        }
    }
}
