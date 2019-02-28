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

    pub fn signal_started(&self) {
    }

    pub fn signal_exited(&self, pid: i32, exit_code: i32) {
    }

    pub fn log_msg(&self, level: LogLevel, msg: String) {
        let msg = format!("[\"call\",\"padre#debugger#Log\",[{},\"{}\"]]",
                          level as i32, msg);
        self.send_msg(msg);
    }

    pub fn jump_to_position(&self, file: String, line: i32) {
    }

    pub fn breakpoint_set(&self, file: String, line: i32) {
    }

    pub fn breakpoint_unset(&self, file: String, line: i32) {
    }

    fn send_msg(&self, msg: String) {
        for mut listener in self.listeners.iter() {
            listener.write(msg.as_bytes()).expect("Can't send to socket");
        }
    }
}
