//! handle server connections

use std::io;

use crate::request::PadreRequest;

use bytes::{Bytes, BytesMut};
use futures::sync::mpsc::{self, UnboundedReceiver};
use tokio::io::ReadHalf;
use tokio::net::TcpStream;
use tokio::prelude::*;

#[derive(Debug)]
pub struct PadreConnection {
    reader: ReadHalf<TcpStream>,
    writer_rx: UnboundedReceiver<Bytes>,
    rd: BytesMut,
}

impl PadreConnection {
    pub fn new(socket: TcpStream) -> Self {
        let (reader, writer) = socket.split();

        let (writer_tx, writer_rx) = mpsc::unbounded();

        tokio::io::copy(writer_tx, writer);

        PadreConnection {
            reader,
            writer_rx,
            rd: BytesMut::new(),
        }
    }

    fn fill_read_buf(&mut self) -> Poll<(), io::Error> {
        loop {
            self.rd.reserve(1024);

            let n = try_ready!(self.reader.read_buf(&mut self.rd));

            if n == 0 {
                return Ok(Async::Ready(()));
            }
        }
    }
}

impl Stream for PadreConnection {
    type Item = PadreRequest;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let sock_closed = self.fill_read_buf()?.is_ready();

        if sock_closed {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_json_good_request_handled() {
    }
}
