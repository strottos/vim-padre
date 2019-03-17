//! notifier functions and traits

// The notifier is responsible for communicating with everything connected to PADRE

use std::io::Write;
use std::net::TcpStream;

pub enum LogLevel {
    CRITICAL = 1,
    ERROR,
    WARN,
    INFO,
    DEBUG
}

//#[cfg(test)]
//mod tests {
//    extern crate simulacrum;
//
//    use simulacrum::*;
//
//    use std::io::{Error, Write};
//
//    create_mock! {
//        impl Write for TcpStreamWriteMock (self) {
//            expect_write("write"):
//                fn write(&mut self, buf: &[u8]) -> Result<usize, Error>;
//            expect_flush("flush"):
//                fn flush(&mut self) -> Result<(), Error>;
//        }
//    }
//
//    #[test]
//    fn test() {
//        let mut notifier = super::Notifier::new();
//        let tcp_stream_mock = TcpStreamWriteMock::new();
//        let tcp_stream_mock_2 = TcpStreamWriteMock::new();
//        notifier.add_listener(Box::new(tcp_stream_mock));
//        notifier.add_listener(Box::new(tcp_stream_mock_2));
//        assert_eq!(notifier.listeners.len(), 2);
//    }
//}

pub struct Notifier {
    listeners: Vec<TcpStream>,
}

impl Notifier {
    pub fn new() -> Notifier {
        Notifier {
            listeners: Vec::new()
        }
    }

    pub fn add_listener(&mut self, stream: TcpStream) {
        self.listeners.push(stream);
    }

    pub fn signal_exited(&self, pid: u32, exit_code: u8) {
        let msg = format!("[\"call\",\"padre#debugger#ProcessExited\",[{},{}]]",
                          exit_code, pid);
        self.send_msg(msg);
    }

    pub fn log_msg(&self, level: LogLevel, msg: String) {
        let msg = format!("[\"call\",\"padre#debugger#Log\",[{},{}]]",
                          level as i32, json::stringify(msg));
        self.send_msg(msg);
    }

    pub fn jump_to_position(&self, file: String, line: u32) {
        let msg = format!("[\"call\",\"padre#debugger#JumpToPosition\",[{},{}]]",
                          json::stringify(file), line);
        self.send_msg(msg);
    }

    pub fn breakpoint_set(&self, file: String, line: u32) {
        let msg = format!("[\"call\",\"padre#debugger#BreakpointSet\",[{},{}]]",
                          json::stringify(file), line);
        self.send_msg(msg);
    }

//    pub fn breakpoint_unset(&self, file: String, line: u32) {
//        let msg = format!("[\"call\",\"padre#debugger#BreakpointUnset\",[{},{}]]",
//                          json::stringify(file), line);
//        self.send_msg(msg);
//    }

    fn send_msg(&self, msg: String) {
        for mut listener in self.listeners.iter() {
            listener.write(msg.as_bytes()).expect("Can't send to socket");
        }
    }
}
