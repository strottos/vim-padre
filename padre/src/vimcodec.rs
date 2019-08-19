//! VIMCodec
//!
//! Rust Tokio Codec for communicating with VIM

use std::collections::HashMap;
use std::io;

use crate::debugger::{DebuggerCmd, DebuggerCmdV1, FileLocation};
use crate::server::{PadreCmd, PadreRequest, PadreSend, RequestCmd};
use crate::util;

use bytes::{BufMut, BytesMut};
use tokio::codec::{Decoder, Encoder};

/// Decodes requests and encodes responses sent by or to VIM over VIM's socket communication
///
/// Given a request of the form
/// ```
/// [1,{"cmd":"breakpoint","file":"test.c","line":1}]
/// ```
/// it decodes this into a PadreRequest with an `id` of `1` and a RequestCmd of `Breakpoint`
/// with the correct file location.
#[derive(Debug)]
pub struct VimCodec {}

impl VimCodec {
    /// Constructor for creating a new VimCodec
    ///
    /// Just creates the object at present.
    pub fn new() -> Self {
        VimCodec {}
    }
}

impl Decoder for VimCodec {
    type Item = PadreRequest;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(None);
        }

        let mut stream = serde_json::Deserializer::from_slice(src).into_iter::<serde_json::Value>();
        let req = &src.clone()[..];

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

                    util::send_error_and_debug(
                        "Must be valid JSON".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );

                    return Ok(None);
                }
            },
            None => {
                unreachable!();
            }
        };

        src.split_to(src.len());

        if !v.is_array() {
            util::send_error_and_debug(
                "Can't read JSON".to_string(),
                format!(
                    "Can't read '{}': Must be an array",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
            return Ok(None);
        }

        if v.as_array().unwrap().len() != 2 {
            util::send_error_and_debug(
                "Can't read JSON".to_string(),
                format!(
                    "Can't read '{}': Array should have 2 elements",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
            return Ok(None);
        }

        let id = v[0].take();
        let id: u64 = match serde_json::from_value(id.clone()) {
            Ok(s) => s,
            Err(e) => {
                util::send_error_and_debug(
                    "Can't read id".to_string(),
                    format!("Can't read '{}': {}", id, e),
                );

                return Ok(None);
            }
        };

        let mut args: HashMap<String, serde_json::Value> =
            match serde_json::from_str(&v[1].take().to_string()) {
                Ok(args) => args,
                Err(e) => {
                    util::send_error_and_debug(
                        "Can't read JSON".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );
                    return Ok(None);
                }
            };

        let cmd: String = match args.remove("cmd") {
            Some(s) => match serde_json::from_value(s) {
                Ok(s) => s,
                Err(e) => {
                    util::send_error_and_debug(
                        "Can't find command".to_string(),
                        format!(
                            "Can't find command '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );
                    return Ok(None);
                }
            },
            None => {
                util::send_error_and_debug(
                    "Can't find command".to_string(),
                    format!(
                        "Can't find command '{}': Need a cmd in 2nd object",
                        String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                    ),
                );
                return Ok(None);
            }
        };

        match &cmd[..] {
            "ping" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestPadreCmd(PadreCmd::Ping),
            ))),
            "pings" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestPadreCmd(PadreCmd::Pings),
            ))),
            "run" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run)),
            ))),
            "stepOver" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::StepOver)),
            ))),
            "stepIn" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::StepIn)),
            ))),
            "continue" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Continue)),
            ))),
            "breakpoint" => {
                let file_location = get_file_location(args);
                match file_location {
                    Some(fl) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Breakpoint(
                            fl,
                        ))),
                    ))),
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }
}

impl Encoder for VimCodec {
    type Item = PadreSend;
    type Error = io::Error;

    fn encode(&mut self, resp: PadreSend, buf: &mut BytesMut) -> Result<(), io::Error> {
        let response = match resp {
            PadreSend::Response(resp) => {
                serde_json::to_string(&(resp.id(), resp.resp())).unwrap() + "\n"
            }
            PadreSend::Notification(notification) => {
                serde_json::to_string(&(
                    "call".to_string(),
                    notification.cmd(),
                    notification.args(),
                ))
                .unwrap()
                    + "\n"
            }
        };

        buf.reserve(response.len());
        buf.put(&response[..]);

        Ok(())
    }
}

/// Get a file location from the arguments
fn get_file_location(mut args: HashMap<String, serde_json::Value>) -> Option<FileLocation> {
    match args.remove("file") {
        Some(s) => match s {
            serde_json::Value::String(s) => match args.remove("line") {
                Some(t) => match t {
                    serde_json::Value::Number(t) => {
                        let t: u64 = match t.as_u64() {
                            Some(t) => t,
                            None => {
                                util::send_error_and_debug(
                                    format!("Badly specified 'line'"),
                                    format!("Badly specified 'line': {}", t),
                                );
                                return None;
                            }
                        };
                        return Some(FileLocation::new(s, t));
                    }
                    _ => {
                        util::send_error_and_debug(
                            "Can't read 'line' argument".to_string(),
                            format!("Can't understand 'line': {}", t),
                        );
                    }
                },
                None => {
                    util::send_error_and_debug(
                        "Can't read 'line' for file location when 'file' specified".to_string(),
                        format!("Can't understand command with file but no line"),
                    );
                }
            },
            _ => {
                util::send_error_and_debug(
                    format!("Can't read 'file' argument"),
                    format!("Can't understand 'file': {}", s),
                );
            }
        },
        None => {}
    };

    return None;
}

#[cfg(test)]
mod tests {
    use crate::debugger::{DebuggerCmd, DebuggerCmdV1, FileLocation};
    use crate::server::{Notification, PadreCmd, PadreRequest, PadreSend, RequestCmd, Response};

    use bytes::{BufMut, BytesMut};
    use tokio::codec::{Decoder, Encoder};

    #[test]
    fn check_simple_json_decoding() {
        let mut codec = super::VimCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(
                123,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
            ),
            padre_request
        );
    }

    #[test]
    fn check_two_simple_json_decoding() {
        let mut codec = super::VimCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(19);
        buf.put(r#"[123,{"cmd":"run"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(
                123,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
            ),
            padre_request
        );

        let mut buf = BytesMut::new();
        buf.reserve(20);
        buf.put(r#"[124,{"cmd":"ping"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(124, RequestCmd::RequestPadreCmd(PadreCmd::Ping)),
            padre_request
        );
    }

    #[test]
    fn check_two_buffers_json_decodings() {
        let mut codec = super::VimCodec::new();
        let mut buf = BytesMut::new();
        buf.reserve(16);
        buf.put(r#"[123,{"cmd":"run"#);

        let padre_request = codec.decode(&mut buf).unwrap();

        assert_eq!(None, padre_request);

        buf.reserve(3);
        buf.put(r#""}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(
                123,
                RequestCmd::RequestDebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
            ),
            padre_request
        );
    }

    #[test]
    fn check_json_encoding_response() {
        let mut codec = super::VimCodec::new();
        let resp = PadreSend::Response(Response::new(123, serde_json::json!({"ping":"pong"})));
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
        let mut codec = super::VimCodec::new();
        let resp = PadreSend::Notification(Notification::new(
            "cmd_test".to_string(),
            vec![serde_json::json!("test"), serde_json::json!(1)],
        ));
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf).unwrap();

        let mut expected = BytesMut::new();
        expected.reserve(31);
        expected.put(r#"["call","cmd_test",["test",1]]"#);
        expected.put("\n");

        assert_eq!(expected, buf);
    }
}
