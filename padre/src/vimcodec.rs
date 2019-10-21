//! VIMCodec
//!
//! Rust Tokio Codec for communicating with VIM

use std::collections::HashMap;
use std::io;

use crate::debugger::{DebuggerCmd, DebuggerCmdV1, FileLocation, Variable};
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

    /// Get and remove a `file location` from the arguments
    fn get_file_location(
        &self,
        args: &mut HashMap<String, serde_json::Value>,
    ) -> Option<FileLocation> {
        match args.remove("file") {
            Some(s) => match s {
                serde_json::Value::String(s) => match args.remove("line") {
                    Some(t) => match t {
                        serde_json::Value::Number(t) => {
                            let t: u64 = match t.as_u64() {
                                Some(t) => t,
                                None => {
                                    util::send_error_and_debug(
                                        &format!("Badly specified 'line'"),
                                        &format!("Badly specified 'line': {}", t),
                                    );
                                    return None;
                                }
                            };
                            return Some(FileLocation::new(s, t));
                        }
                        _ => {
                            util::send_error_and_debug(
                                "Can't read 'line' argument",
                                &format!("Can't understand 'line': {}", t),
                            );
                        }
                    },
                    None => {
                        util::send_error_and_debug(
                            "Can't understand request",
                            "Need to specify a line number",
                        );
                    }
                },
                _ => {
                    util::send_error_and_debug(
                        &format!("Can't read 'file' argument"),
                        &format!("Can't understand 'file': {}", s),
                    );
                }
            },
            None => {
                util::send_error_and_debug(
                    "Can't understand request",
                    "Need to specify a file name",
                );
            }
        };

        None
    }

    /// Get and remove a `variable` from the arguments passed
    fn get_variable(&self, args: &mut HashMap<String, serde_json::Value>) -> Option<Variable> {
        match args.remove("variable") {
            Some(s) => match s {
                serde_json::Value::String(s) => Some(Variable::new(s)),
                _ => {
                    util::send_error_and_debug(
                        "Badly specified 'variable'",
                        &format!("Badly specified 'variable': {}", s),
                    );
                    None
                }
            },
            None => {
                util::send_error_and_debug(
                    "Can't understand request",
                    "Need to specify a variable name",
                );
                None
            }
        }
    }

    /// Get and remove a `variable` and `value` from the arguments passed and pass back a
    /// `Variable`
    fn get_variable_with_value(
        &self,
        args: &mut HashMap<String, serde_json::Value>,
    ) -> Option<Variable> {
        let variable = self.get_variable(args);
        let variable = match variable {
            Some(mut v) => {
                let var = &mut v;
                match args.remove("value") {
                    Some(s) => match s {
                        serde_json::Value::String(s) => {
                            let mut variable = Variable::new(var.get_name());
                            variable.set_value(format!("\"{}\"", s));
                            Some(variable)
                        }
                        serde_json::Value::Number(s) => {
                            let mut variable = Variable::new(var.get_name());
                            variable.set_value(format!("{}", s));
                            Some(variable)
                        }
                        _ => {
                            util::send_error_and_debug(
                                "Badly specified 'variable'",
                                &format!("Badly specified 'variable': {}", s),
                            );
                            None
                        }
                    },
                    None => {
                        util::send_error_and_debug(
                            "Can't understand request",
                            "Need to specify a variable name",
                        );
                        None
                    }
                }
            }
            None => None,
        };
        variable
    }

    /// Get and remove the key specified from the arguments as a String
    fn get_string(
        &self,
        key: &str,
        args: &mut HashMap<String, serde_json::Value>,
    ) -> Option<String> {
        match args.remove(key) {
            Some(s) => match s {
                serde_json::Value::String(s) => Some(s),
                _ => {
                    util::send_error_and_debug(
                        &format!("Badly specified string '{}'", key),
                        &format!("Badly specified string '{}': {}", key, s),
                    );
                    None
                }
            },
            None => {
                util::send_error_and_debug(
                    "Can't understand request",
                    &format!("Need to specify a '{}'", key),
                );
                None
            }
        }
    }

    /// Get and remove the key specified from the arguments as an i64
    fn get_i64(&self, key: &str, args: &mut HashMap<String, serde_json::Value>) -> Option<i64> {
        match args.remove(key) {
            Some(k) => match k.clone() {
                serde_json::Value::Number(n) => match n.as_i64() {
                    Some(i) => Some(i),
                    None => {
                        util::send_error_and_debug(
                            &format!("Badly specified 64-bit integer '{}'", key),
                            &format!("Badly specified 64-bit integer '{}': {}", key, &k),
                        );
                        None
                    }
                },
                _ => {
                    util::send_error_and_debug(
                        &format!("Badly specified 64-bit integer '{}'", key),
                        &format!("Badly specified 64-bit integer '{}': {}", key, &k),
                    );
                    None
                }
            },
            None => {
                util::send_error_and_debug(
                    "Can't understand request",
                    &format!("Need to specify a '{}'", key),
                );
                None
            }
        }
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
                        "Must be valid JSON",
                        &format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    );

                    return Ok(None);
                }
            },
            None => {
                println!("If this line prints and problems occur please raise a bug report");
                return Ok(None);
            }
        };

        src.split_to(src.len());

        if !v.is_array() {
            util::send_error_and_debug(
                "Can't read JSON",
                &format!(
                    "Can't read '{}': Must be an array",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
            return Ok(None);
        }

        if v.as_array().unwrap().len() != 2 {
            util::send_error_and_debug(
                "Can't read JSON",
                &format!(
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
                util::send_error_and_debug("Can't read id", &format!("Can't read '{}': {}", id, e));

                return Ok(None);
            }
        };

        let mut args: HashMap<String, serde_json::Value> =
            match serde_json::from_str(&v[1].take().to_string()) {
                Ok(args) => args,
                Err(e) => {
                    util::send_error_and_debug(
                        "Can't read JSON",
                        &format!(
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
                        "Can't find command",
                        &format!(
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
                    "Can't find command",
                    &format!(
                        "Can't find command '{}': Need a cmd in 2nd object",
                        String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                    ),
                );
                return Ok(None);
            }
        };

        let ret = match &cmd[..] {
            "ping" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::PadreCmd(PadreCmd::Ping),
            ))),
            "pings" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::PadreCmd(PadreCmd::Pings),
            ))),
            "run" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run)),
            ))),
            "stepOver" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::StepOver)),
            ))),
            "stepIn" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::StepIn)),
            ))),
            "continue" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Continue)),
            ))),
            "breakpoint" => {
                let file_location = self.get_file_location(&mut args);
                match file_location {
                    Some(fl) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Breakpoint(fl))),
                    ))),
                    None => return Ok(None),
                }
            }
            "print" => {
                let variable = self.get_variable(&mut args);
                match variable {
                    Some(v) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Print(v))),
                    ))),
                    None => return Ok(None),
                }
            }
            "set" => {
                let variable = self.get_variable_with_value(&mut args);
                match variable {
                    Some(v) => {
                        return Ok(Some(PadreRequest::new(
                            id,
                            RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Set(v))),
                        )));
                    }
                    None => return Ok(None),
                }
            }
            "getConfig" => {
                let key = self.get_string("key", &mut args);
                match key {
                    Some(k) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::PadreCmd(PadreCmd::GetConfig(k)),
                    ))),
                    None => return Ok(None),
                }
            }
            "setConfig" => {
                let key = self.get_string("key", &mut args);
                match key {
                    Some(k) => {
                        let value = self.get_i64("value", &mut args);
                        match value {
                            Some(v) => Ok(Some(PadreRequest::new(
                                id,
                                RequestCmd::PadreCmd(PadreCmd::SetConfig(k, v)),
                            ))),
                            None => return Ok(None),
                        }
                    }
                    None => return Ok(None),
                }
            }
            _ => {
                util::send_error_and_debug(
                    "Command unknown",
                    &format!("Command unknown: '{}'", cmd),
                );
                Ok(None)
            }
        };

        match args.is_empty() {
            true => {}
            false => {
                let mut args_left: Vec<String> = args.iter().map(|(key, _)| key.clone()).collect();
                args_left.sort();
                util::send_error_and_debug(
                    "Bad arguments",
                    &format!("Bad arguments: {:?}", args_left),
                );
                return Ok(None);
            }
        };

        ret
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

#[cfg(test)]
mod tests {
    use crate::debugger::{DebuggerCmd, DebuggerCmdV1};
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
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
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
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
            ),
            padre_request
        );

        let mut buf = BytesMut::new();
        buf.reserve(20);
        buf.put(r#"[124,{"cmd":"ping"}]"#);

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            PadreRequest::new(124, RequestCmd::PadreCmd(PadreCmd::Ping)),
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
                RequestCmd::DebuggerCmd(DebuggerCmd::V1(DebuggerCmdV1::Run))
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
