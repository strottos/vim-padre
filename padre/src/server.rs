//! handle server connections

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreDebugger;
use crate::notifier::Notifier;
use crate::request::{PadreRequest, PadreResponse};

use bytes::{BufMut, Bytes, BytesMut};
//use futures::sync::mpsc::{self, UnboundedReceiver};
use tokio::codec::{Decoder, Encoder, Framed};
use tokio::net::TcpStream;
use tokio::prelude::stream::{SplitSink, SplitStream};
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Receiver, Sender};

pub fn process_connection(
    socket: TcpStream,
    debugger: Arc<Mutex<PadreDebugger>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let addr = socket.peer_addr().unwrap();

    let (tx, rx) = PadreCodec::new().framed(socket).split();

    let (mut send_tx, send_rx) = mpsc::channel(32);

    notifier.lock().unwrap().add_listener(send_tx.clone(), addr);

    // TODO:
    //if debugger.has_started() {
    notifier.lock().unwrap().signal_started();
    //}

    tokio::spawn(
        tx.send_all(send_rx.map_err(|e| {
            eprintln!("failed to retrieve message to send: {}", e);
            io::Error::new(io::ErrorKind::Other, e)
        }))
        .then(|res| {
            if let Err(e) = res {
                eprintln!("failed to send data to socket; error = {:?}", e);
            }

            Ok(())
        }),
    );

    tokio::spawn(
        rx.and_then(move |req| {
            let debugger = debugger.clone();
            respond(req, debugger)
        })
        .for_each(move |resp| {
            send_tx.try_send(resp).unwrap();
            Ok(())
        })
        .map_err(|e| eprintln!("failed to accept socket; error = {:?}", e)),
    );
}

fn respond(
    req: PadreRequest,
    debugger: Arc<Mutex<PadreDebugger>>,
) -> Box<dyn Future<Item = PadreResponse, Error = io::Error> + Send> {
    let f = future::lazy(move || {
        let json_response = match req.cmd() {
            "ping" => debugger.lock().unwrap().ping(),
            "pings" => debugger.lock().unwrap().pings(),
            _ => unreachable!(),
        };

        match json_response {
            Ok(resp) => Ok(PadreResponse::Response(req.id(), resp)),
            Err(_) => {
                println!("TODO - implement");
                unreachable!();
            }
        }
    });

    Box::new(f)
}

//#[derive(Debug)]
//pub struct PadreConnection {
//    reader: SplitStream<Framed<TcpStream, PadreCodec>>,
//    writer_rx: UnboundedReceiver<Bytes>,
//    rd: BytesMut,
//}
//
//impl PadreConnection {
//    pub fn new(socket: TcpStream) -> Self {
//        let (writer_tx, writer_rx) = mpsc::unbounded();
//
//        let (writer, reader) = PadreCodec::new().framed(socket).split();
//
//        PadreConnection {
//            reader,
//            writer_rx,
//            rd: BytesMut::new(),
//        }
//    }
//    //
//    //    fn fill_read_buf(&mut self) -> Poll<(), io::Error> {
//    //        loop {
//    //            self.rd.reserve(1024);
//    //
//    //            let n = try_ready!(self.reader.read_buf(&mut self.rd));
//    //
//    //            if n == 0 {
//    //                return Ok(Async::Ready(()));
//    //            }
//    //        }
//    //    }
//}
//
//impl Stream for PadreConnection {
//    type Item = PadreRequest;
//    type Error = io::Error;
//
//    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
//        //        let sock_closed = self.fill_read_buf()?.is_ready();
//        //
//        //        if sock_closed {
//        //            Ok(Async::Ready(None))
//        //        } else {
//        Ok(Async::NotReady)
//        //        }
//    }
//}

#[derive(Debug)]
struct PadreCodec {
    // Track a list of places we should try from in case one of the sends cut off
    try_from: Vec<usize>,
}

impl PadreCodec {
    fn new() -> Self {
        let try_from = vec![0];
        PadreCodec { try_from }
    }
}

impl Decoder for PadreCodec {
    type Item = PadreRequest;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut v: serde_json::Value = serde_json::json!(null);

        // If we match a full json entry from any point we assume we're good
        for from in self.try_from.iter() {
            v = match serde_json::from_slice(&src[*from..]) {
                Ok(s) => s,
                Err(err) => {
                    if err.is_eof() || err.is_syntax() {
                        serde_json::json!(null)
                    } else {
                        println!("TODO - Handle error: {:?}", err);
                        println!("Stream: {:?}", src);
                        unreachable!();
                    }
                }
            };

            if !v.is_null() {
                break;
            }
        }

        if v.is_null() {
            self.try_from.push(src.len());
            return Ok(None);
        }

        let id: u32 = serde_json::from_value(v[0].take()).unwrap();
        let cmd: String = serde_json::from_value(v[1]["cmd"].take()).unwrap();
        let padre_request: PadreRequest = PadreRequest::new(id, cmd);

        src.split_to(src.len());
        self.try_from = vec![0];

        Ok(Some(padre_request))
    }
}

impl Encoder for PadreCodec {
    type Item = PadreResponse;
    type Error = io::Error;

    fn encode(&mut self, resp: PadreResponse, buf: &mut BytesMut) -> Result<(), io::Error> {
        let response = match resp {
            PadreResponse::Response(id, json) => serde_json::to_string(&(id, json)).unwrap(),
            PadreResponse::Notify(cmd, args) => {
                serde_json::to_string(&("call".to_string(), cmd, args)).unwrap()
            }
        };

        buf.reserve(response.len());
        buf.put(&response[..]);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::request::{PadreRequest, PadreResponse};
    use bytes::{BufMut, Bytes, BytesMut};
    use tokio::codec::{Decoder, Encoder};

    #[test]
    fn check_single_json_decoding() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(PadreRequest::new(123, "run".to_string()), padre_request);
    }

    #[test]
    fn check_two_json_decodings() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(PadreRequest::new(123, "run".to_string()), padre_request);

        buf.reserve(19);
        buf.put(r#"[124,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(PadreRequest::new(124, "run".to_string()), padre_request);
    }

    #[test]
    fn check_two_buffers_json_decodings() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(16);
        buf.put(r#"[123,{"cmd":"run"#);

        let padre_request = codec.decode(&mut buf).unwrap();

        assert_eq!(None, padre_request);

        buf.reserve(3);
        buf.put(r#""}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(PadreRequest::new(123, "run".to_string()), padre_request);
    }

    #[test]
    fn check_bad_then_good_json_decodings() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(16);
        buf.put(r#"[123,{"cmd":"run"#);

        let padre_request = codec.decode(&mut buf).unwrap();

        assert_eq!(None, padre_request);

        buf.reserve(19);
        buf.put(r#"[124,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(PadreRequest::new(124, "run".to_string()), padre_request);
    }

    #[test]
    fn check_json_encoding_response() {
        let mut codec = super::PadreCodec::new();
        let resp = PadreResponse::Response(123, serde_json::json!({"ping":"pong"}));
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf);

        let mut expected = BytesMut::new();
        expected.reserve(21);
        expected.put(r#"[123,{"ping":"pong"}]"#);

        assert_eq!(expected, buf);
    }

    #[test]
    fn check_json_encoding_notify() {
        let mut codec = super::PadreCodec::new();
        let resp = PadreResponse::Notify(
            "cmd_test".to_string(),
            vec!["test".to_string(), "1".to_string()],
        );
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf);

        let mut expected = BytesMut::new();
        expected.reserve(32);
        expected.put(r#"["call","cmd_test",["test","1"]]"#);

        assert_eq!(expected, buf);
    }
}
