//! Codec for communicating with VIM

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};
use crate::server::{Request, RequestCmd, Response};

use bytes::{BufMut, BytesMut};
use tokio::codec::{Decoder, Encoder};

#[derive(Debug)]
pub struct VimCodec {
    // Track a list of places we should try from in case one of the sends cut off
    notifier: Arc<Mutex<Notifier>>,
    addr: SocketAddr,
}

impl VimCodec {
    pub fn new(notifier: Arc<Mutex<Notifier>>, addr: SocketAddr) -> Self {
        VimCodec { notifier, addr }
    }
}

impl Drop for VimCodec {
    fn drop(&mut self) {
        self.notifier.lock().unwrap().remove_listener(&self.addr);
    }
}

impl Decoder for VimCodec {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(None);
        }

        let mut stream = serde_json::Deserializer::from_slice(src).into_iter::<serde_json::Value>();
        let req = src.clone();

        let mut v = match stream.next() {
            Some(s) => match s {
                Ok(t) => t,
                Err(e) => {
                    match e.classify() {
                        serde_json::error::Category::Io => {
                            println!("IO: {:?}", req);
                        }
                        serde_json::error::Category::Syntax => {}
                        serde_json::error::Category::Data => {
                            println!("Data: {:?}", req);
                        }
                        serde_json::error::Category::Eof => {
                            return Ok(None);
                        }
                    };

                    src.split_to(src.len());

                    return send_error_and_debug(
                        self.notifier.clone(),
                        "Must be valid JSON".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );
                }
            },
            None => {
                unreachable!();
            }
        };

        src.split_to(src.len());

        if !v.is_array() {
            return send_error_and_debug(
                self.notifier.clone(),
                "Can't read JSON".to_string(),
                format!(
                    "Can't read '{}': Must be an array",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
        }

        if v.as_array().unwrap().len() != 2 {
            return send_error_and_debug(
                self.notifier.clone(),
                "Can't read JSON".to_string(),
                format!(
                    "Can't read '{}': Array should have 2 elements",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
        }

        let id = v[0].take();
        let id: u64 = match serde_json::from_value(id.clone()) {
            Ok(s) => s,
            Err(e) => {
                return send_error_and_debug(
                    self.notifier.clone(),
                    "Can't read id".to_string(),
                    format!("Can't read '{}': {}", id, e),
                );
            }
        };

        let mut args: HashMap<String, serde_json::Value> =
            match serde_json::from_str(&v[1].take().to_string()) {
                Ok(args) => args,
                Err(e) => {
                    return send_error_and_debug(
                        self.notifier.clone(),
                        "Can't read JSON".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );
                }
            };

        let cmd = match args.remove("cmd") {
            Some(s) => s,
            None => {
                return send_error_and_debug(
                    self.notifier.clone(),
                    "Can't find command".to_string(),
                    format!(
                        "Can't find command '{}': Need a cmd in 2nd object",
                        String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                    ),
                );
            }
        };

        let cmd: String = match serde_json::from_value(cmd) {
            Ok(s) => s,
            Err(e) => {
                return send_error_and_debug(
                    self.notifier.clone(),
                    "Can't find command".to_string(),
                    format!(
                        "Can't find command '{}': {}",
                        String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                        e
                    ),
                );
            }
        };

        let file_location: Option<(String, u64)> = match args.remove("file") {
            Some(s) => match s {
                serde_json::Value::String(s) => match args.remove("line") {
                    Some(t) => match t {
                        serde_json::Value::Number(t) => {
                            let t: u64 = match t.as_u64() {
                                Some(t) => t,
                                None => {
                                    return send_error_and_debug(
                                        self.notifier.clone(),
                                        format!("Badly specified 'line'"),
                                        format!("Badly specified 'line': {}", t),
                                    );
                                }
                            };
                            Some((s, t))
                        }
                        _ => {
                            return send_error_and_debug(
                                self.notifier.clone(),
                                "Can't read 'line' argument".to_string(),
                                format!("Can't understand 'line': {}", t),
                            );
                        }
                    },
                    None => {
                        return send_error_and_debug(
                            self.notifier.clone(),
                            "Can't read 'line' for file location when 'file' specified".to_string(),
                            format!("Can't understand command with file but no line: '{}'", cmd),
                        );
                    }
                },
                _ => {
                    return send_error_and_debug(
                        self.notifier.clone(),
                        format!("Can't read 'file' argument"),
                        format!("Can't understand 'file': {}", s),
                    );
                }
            },
            None => None,
        };

        let variable: Option<String> = match args.remove("variable") {
            Some(s) => match s {
                serde_json::Value::String(s) => Some(s),
                _ => {
                    return send_error_and_debug(
                        self.notifier.clone(),
                        format!("Badly specified 'variable'"),
                        format!("Badly specified 'variable': {}", s),
                    );
                }
            },
            None => None,
        };

        let cmd: RequestCmd = match file_location {
            Some(s) => RequestCmd::CmdWithFileLocation(cmd, s.0, s.1),
            None => match variable {
                Some(s) => RequestCmd::CmdWithVariable(cmd, s),
                None => RequestCmd::Cmd(cmd),
            },
        };

        if !args.is_empty() {
            let mut args_left: Vec<String> = args.iter().map(|(key, _)| key.clone()).collect();
            args_left.sort();
            return send_error_and_debug(
                self.notifier.clone(),
                "Bad arguments".to_string(),
                format!("Bad arguments: {:?}", args_left),
            );
        }

        let padre_request: Request = Request::new(id, cmd);

        Ok(Some(padre_request))
    }
}

impl Encoder for VimCodec {
    type Item = Response;
    type Error = io::Error;

    fn encode(&mut self, resp: Response, buf: &mut BytesMut) -> Result<(), io::Error> {
        let response = match resp {
            Response::Response(id, json) => serde_json::to_string(&(id, json)).unwrap() + "\n",
            Response::Notify(cmd, args) => {
                serde_json::to_string(&("call".to_string(), cmd, args)).unwrap() + "\n"
            }
        };

        buf.reserve(response.len());
        buf.put(&response[..]);

        Ok(())
    }
}

fn send_error_and_debug(
    notifier: Arc<Mutex<Notifier>>,
    err_msg: String,
    debug_msg: String,
) -> Result<Option<Request>, io::Error> {
    notifier.lock().unwrap().log_msg(LogLevel::ERROR, err_msg);
    notifier.lock().unwrap().log_msg(LogLevel::DEBUG, debug_msg);
    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::{Arc, Mutex};

    use crate::notifier::Notifier;
    use crate::server::{Request, RequestCmd, Response};

    use bytes::{BufMut, BytesMut};
    use tokio::codec::{Decoder, Encoder};

    fn get_notifier() -> Arc<Mutex<Notifier>> {
        Arc::new(Mutex::new(Notifier::new()))
    }

    #[test]
    fn check_simple_json_decoding() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(123, RequestCmd::Cmd("run".to_string())),
            padre_request
        );
    }

    #[test]
    fn check_two_json_decodings() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(123, RequestCmd::Cmd("run".to_string())),
            padre_request
        );

        buf.reserve(19);
        buf.put(r#"[124,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(124, RequestCmd::Cmd("run".to_string())),
            padre_request
        );
    }

    #[test]
    fn check_two_buffers_json_decodings() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let mut buf = BytesMut::new();
        buf.reserve(16);
        buf.put(r#"[123,{"cmd":"run"#);

        let padre_request = codec.decode(&mut buf).unwrap();

        assert_eq!(None, padre_request);

        buf.reserve(3);
        buf.put(r#""}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(123, RequestCmd::Cmd("run".to_string())),
            padre_request
        );
    }

    //#[test]
    //fn check_bad_then_good_json_decodings() {
    //    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    //    let mut codec = super::VimCodec::new(get_notifier(), addr);
    //    let mut buf = BytesMut::new();
    //    buf.reserve(16);
    //    buf.put(r#"[123,{"cmd":"run"#);

    //    let padre_request = codec.decode(&mut buf).unwrap();

    //    assert_eq!(None, padre_request);

    //    buf.reserve(19);
    //    buf.put(r#"[124,{"cmd":"run"}]"#);

    //    let padre_request = codec.decode(&mut buf).unwrap();

    //    println!("PADRE Request: {:?}", padre_request);

    //    assert_eq!(
    //        Request::new(124, RequestCmd::Cmd("run".to_string())),
    //        padre_request.unwrap()
    //    );
    //}

    #[test]
    fn check_json_decoding_with_file_location() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let mut buf = BytesMut::new();
        buf.reserve(53);
        buf.put(r#"[123,{"cmd":"breakpoint","file":"test.c","line":125}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(
                123,
                RequestCmd::CmdWithFileLocation(
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
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let mut buf = BytesMut::new();
        buf.reserve(36);
        buf.put(r#"[123,{"cmd":"print","variable":"a"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            Request::new(
                123,
                RequestCmd::CmdWithVariable("print".to_string(), "a".to_string())
            ),
            padre_request
        );
    }

    #[test]
    fn check_json_encoding_response() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let resp = Response::Response(123, serde_json::json!({"ping":"pong"}));
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf).unwrap();

        let mut expected = BytesMut::new();
        expected.reserve(22);
        expected.put(r#"[123,{"ping":"pong"}]"#);
        expected.put("\n");

        assert_eq!(expected, buf);
    }

    #[test]
    fn check_json_encoding_notify() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut codec = super::VimCodec::new(get_notifier(), addr);
        let resp = Response::Notify(
            "cmd_test".to_string(),
            vec![serde_json::json!("test"), serde_json::json!(1)],
        );
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf).unwrap();

        let mut expected = BytesMut::new();
        expected.reserve(31);
        expected.put(r#"["call","cmd_test",["test",1]]"#);
        expected.put("\n");

        assert_eq!(expected, buf);
    }
}
