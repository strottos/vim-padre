//! handle server connections

use std::io;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreDebugger;
use crate::notifier::{LogLevel, Notifier};
use crate::request::{PadreRequest, PadreRequestCmd, PadreResponse, RequestError};

use bytes::{BufMut, BytesMut};
use tokio::codec::{Decoder, Encoder};
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::sync::mpsc;

pub fn process_connection(
    socket: TcpStream,
    debugger: Arc<Mutex<PadreDebugger>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let addr = socket.peer_addr().unwrap();

    let (request_tx, request_rx) = PadreCodec::new(notifier.clone()).framed(socket).split();

    let (connection_tx, connection_rx) = mpsc::channel(1);

    notifier
        .lock()
        .unwrap()
        .add_listener(connection_tx.clone(), addr);

    if debugger.lock().unwrap().has_started() {
        notifier.lock().unwrap().signal_started();
    }

    tokio::spawn(
        request_tx
            .send_all(connection_rx.map_err(|e| {
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
        request_rx
            .and_then(move |req| {
                let debugger = debugger.clone();
                respond(req, debugger)
            })
            .for_each(move |resp| {
                tokio::spawn(
                    connection_tx
                        .clone()
                        .send(resp)
                        .map(|_| {})
                        .map_err(|e| println!("Error sending to LLDB: {}", e)),
                );
                Ok(())
            })
            .map_err(|e| eprintln!("failed to accept socket; error = {:?}", e)),
    );
}

fn respond(
    req: PadreRequest,
    debugger: Arc<Mutex<PadreDebugger>>,
) -> Box<dyn Future<Item = PadreResponse, Error = io::Error> + Send> {
    let json_response = match req.cmd() {
        PadreRequestCmd::Cmd(s) => {
            let s: &str = s;
            match s {
                "ping" => debugger.lock().unwrap().ping(),
                "pings" => debugger.lock().unwrap().pings(),
                _ => return respond_debugger(req, debugger),
            }
        }
        _ => return respond_debugger(req, debugger),
    };

    let f = future::lazy(move || match json_response {
        Ok(resp) => Ok(PadreResponse::Response(req.id(), resp)),
        Err(_) => {
            println!("TODO - Implement");
            panic!("ERROR4");
        }
    });

    Box::new(f)
}

fn respond_debugger(
    req: PadreRequest,
    debugger: Arc<Mutex<PadreDebugger>>,
) -> Box<dyn Future<Item = PadreResponse, Error = io::Error> + Send> {
    let id = req.id();

    // TODO: Timeouts
    let f = debugger
        .lock()
        .unwrap()
        .handle(req)
        .then(move |resp| Ok(PadreResponse::Response(id, resp.unwrap())));

    return Box::new(f);
}

#[derive(Debug)]
struct PadreCodec {
    // Track a list of places we should try from in case one of the sends cut off
    notifier: Arc<Mutex<Notifier>>,
    try_from: Vec<usize>,
}

impl PadreCodec {
    fn new(notifier: Arc<Mutex<Notifier>>) -> Self {
        let try_from = vec![0];
        PadreCodec { notifier, try_from }
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
            println!("HERE1 {:?}", src);
            if src.len() > 0 {
                self.try_from.push(src.len());
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::ERROR, "Must be valid JSON".to_string());
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::DEBUG, format!("Can't read '{}'", String::from_utf8_lossy(&src[..]).trim_matches(char::from(0))));
            }
            return Ok(None);
        }

        let req = src.clone();

        src.split_to(src.len());
        self.try_from = vec![0];

        if !v.is_array() {
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::ERROR, "Can't read JSON".to_string());
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::DEBUG, format!("Can't read '{}': Must be an array", String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))));
            return Ok(None);
        }

        if v.as_array().unwrap().len() != 2 {
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::ERROR, "Can't read JSON".to_string());
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::DEBUG, format!("Can't read '{}': Array should have 2 elements", String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))));
            return Ok(None);
        }

        let id = v[0].take();
        let id: u64 = match serde_json::from_value(id.clone()) {
            Ok(s) => s,
            Err(err) => {
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::ERROR, "Can't read id".to_string());
                self.notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::DEBUG, format!("Can't read '{}': {}", id, err));
                return Ok(None);
            }
        };

        let mut args = v[1].take();

        if !args.is_object() {
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::ERROR, "Can't read JSON".to_string());
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::DEBUG, format!("Can't read '{}': 2nd element must be an object", String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))));
            return Ok(None);
        }

        let cmd: String = match serde_json::from_value(args["cmd"].take()) {
            Ok(s) => s,
            Err(err) => {
                println!("TODO - Implement: {}", err);
                panic!("ERROR1");
            }
        };

        let file_location: Option<(String, u64)> = match serde_json::from_value(args["file"].take()) {
            Ok(s) => match serde_json::from_value(v[1]["line"].take()) {
                Ok(t) => {
                    let t: u64 = t;
                    Some((s, t))
                }
                Err(err) => {
                    println!("TODO - Implement: {}", err);
                    panic!("ERROR2");
                }
            },
            Err(err) => {
                println!("ERROR Not handling {:?}", err);
                None
            }
        };

        let variable: Option<String> = match serde_json::from_value(v[1]["variable"].take()) {
            Ok(s) => Some(s),
            Err(err) => {
                println!("ERROR Not handling {:?}", err);
                None
            }
        };

        let cmd: PadreRequestCmd = match file_location {
            Some(s) => PadreRequestCmd::CmdWithFileLocation(cmd, s.0, s.1),
            None => match variable {
                Some(s) => PadreRequestCmd::CmdWithVariable(cmd, s),
                None => PadreRequestCmd::Cmd(cmd),
            },
        };

        let padre_request: PadreRequest = PadreRequest::new(id, cmd);
        // TODO: If anything left in v error

        Ok(Some(padre_request))
    }
}

impl Encoder for PadreCodec {
    type Item = PadreResponse;
    type Error = io::Error;

    fn encode(&mut self, resp: PadreResponse, buf: &mut BytesMut) -> Result<(), io::Error> {
        println!("HERE2");
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
    use crate::request::{PadreRequest, PadreRequestCmd, PadreResponse};
    use bytes::{BufMut, Bytes, BytesMut};
    use tokio::codec::{Decoder, Encoder};

    #[test]
    fn check_simple_json_decoding() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(123, PadreRequestCmd::Cmd("run".to_string())),
            padre_request
        );
    }

    #[test]
    fn check_two_json_decodings() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(123, PadreRequestCmd::Cmd("run".to_string())),
            padre_request
        );

        buf.reserve(19);
        buf.put(r#"[124,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(124, PadreRequestCmd::Cmd("run".to_string())),
            padre_request
        );
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

        assert_eq!(
            PadreRequest::new(123, PadreRequestCmd::Cmd("run".to_string())),
            padre_request
        );
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

        assert_eq!(
            PadreRequest::new(124, PadreRequestCmd::Cmd("run".to_string())),
            padre_request
        );
    }

    #[test]
    fn check_json_decoding_with_file_location() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(53);
        buf.put(r#"[123,{"cmd":"breakpoint","file":"test.c","line":125}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(
                123,
                PadreRequestCmd::CmdWithFileLocation(
                    "breakpoint".to_string(),
                    "test.c".to_string(),
                    125
                )
            ),
            padre_request
        );
    }

    #[test]
    fn check_json_decoding_with_variable() {
        let mut codec = super::PadreCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(36);
        buf.put(r#"[123,{"cmd":"print","variable":"a"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(
                123,
                PadreRequestCmd::CmdWithVariable("print".to_string(), "a".to_string())
            ),
            padre_request
        );
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
            vec![serde_json::json!("test"), serde_json::json!(1)],
        );
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf);

        let mut expected = BytesMut::new();
        expected.reserve(32);
        expected.put(r#"["call","cmd_test",["test",1]]"#);

        assert_eq!(expected, buf);
    }
}
