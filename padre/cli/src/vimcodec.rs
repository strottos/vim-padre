//! VIMCodec
//!
//! Rust Tokio Codec for communicating with VIM

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::server::{PadreCmd, PadreRequest, PadreSend, RequestCmd};
use padre_core::debugger::{DebuggerCmd, FileLocation, Variable};
use padre_core::server::{PadreError, PadreErrorKind};

use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug)]
pub struct PadreErrorWithId {
    padre_error: PadreError,
    id: u64,
}

impl PadreErrorWithId {
    pub fn new(kind: PadreErrorKind, id: u64, error_string: String, debug_string: String) -> Self {
        PadreErrorWithId {
            id,
            padre_error: PadreError::new(kind, error_string, debug_string),
        }
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_error_string(&self) -> &str {
        &self.padre_error.get_error_string()
    }

    pub fn get_debug_string(&self) -> &str {
        &self.padre_error.get_debug_string()
    }
}

impl From<io::Error> for PadreErrorWithId {
    fn from(err: io::Error) -> PadreErrorWithId {
        PadreErrorWithId::new(
            PadreErrorKind::GenericError,
            0,
            "Generic error".to_string(),
            format!("Generic error {}", err),
        )
    }
}

type Result<T> = std::result::Result<T, PadreErrorWithId>;

/// Decodes requests and encodes responses sent by or to VIM over VIM's socket communication
///
/// Given a request of the form
/// ```
/// [1,{"cmd":"breakpoint","file":"test.c","line":1}]
/// ```
/// it decodes this into a PadreRequest with an `id` of `1` and a RequestCmd of `Breakpoint`
/// with the correct file location.
#[derive(Debug)]
pub struct VimCodec<'a> {
    config: Arc<Mutex<Config<'a>>>,
}

impl<'a> VimCodec<'a> {
    /// Constructor for creating a new VimCodec
    ///
    /// Just creates the object at present.
    pub fn new(config: Arc<Mutex<Config<'a>>>) -> Self {
        VimCodec { config }
    }

    /// Get and remove a `file location` from the arguments
    fn get_file_location(
        &self,
        args: &mut HashMap<String, serde_json::Value>,
        id: u64,
    ) -> Result<FileLocation> {
        match args.remove("file") {
            Some(s) => match s {
                serde_json::Value::String(s) => match args.remove("line") {
                    Some(t) => match t {
                        serde_json::Value::Number(t) => {
                            let t: u64 = match t.as_u64() {
                                Some(t) => t,
                                None => {
                                    return Err(PadreErrorWithId::new(
                                        PadreErrorKind::RequestSyntaxError,
                                        id,
                                        "Badly specified 'line'".to_string(),
                                        format!("Badly specified 'line': {}", t),
                                    ));
                                }
                            };
                            Ok(FileLocation::new(s, t))
                        }
                        _ => Err(PadreErrorWithId::new(
                            PadreErrorKind::RequestSyntaxError,
                            id,
                            "Can't read 'line' argument".to_string(),
                            format!("Can't understand 'line': {}", t),
                        )),
                    },
                    None => Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        id,
                        "Can't understand request".to_string(),
                        "Need to specify a line number".to_string(),
                    )),
                },
                _ => Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    "Can't read 'file' argument".to_string(),
                    format!("Can't understand 'file': {}", s),
                )),
            },
            None => Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Can't understand request".to_string(),
                "Need to specify a file name".to_string(),
            )),
        }
    }

    /// Get and remove a `variable` from the arguments passed
    fn get_variable(
        &self,
        args: &mut HashMap<String, serde_json::Value>,
        id: u64,
    ) -> Result<Variable> {
        match args.remove("variable") {
            Some(s) => match s {
                serde_json::Value::String(s) => Ok(Variable::new(s)),
                _ => Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    "Badly specified 'variable'".to_string(),
                    format!("Badly specified 'variable': {}", s),
                )),
            },
            None => Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Can't understand request".to_string(),
                "Need to specify a variable name".to_string(),
            )),
        }
    }

    /// Get and remove the key specified from the arguments as a String
    fn get_string(
        &self,
        key: &str,
        args: &mut HashMap<String, serde_json::Value>,
        id: u64,
    ) -> Result<String> {
        match args.remove(key) {
            Some(s) => match s {
                serde_json::Value::String(s) => Ok(s),
                _ => Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    format!("Badly specified string '{}'", key),
                    format!("Badly specified string '{}': {}", key, s),
                )),
            },
            None => Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Can't understand request".to_string(),
                format!("Need to specify a '{}'", key),
            )),
        }
    }

    /// Get and remove the key specified from the arguments as an i64
    fn get_i64(
        &self,
        key: &str,
        args: &mut HashMap<String, serde_json::Value>,
        id: u64,
    ) -> Result<i64> {
        match args.remove(key) {
            Some(k) => match k.clone() {
                serde_json::Value::Number(n) => match n.as_i64() {
                    Some(i) => Ok(i),
                    None => Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        id,
                        format!("Badly specified 64-bit integer '{}'", key),
                        format!("Badly specified 64-bit integer '{}': {}", key, &k),
                    )),
                },
                _ => Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    format!("Badly specified 64-bit integer '{}'", key),
                    format!("Badly specified 64-bit integer '{}': {}", key, &k),
                )),
            },
            None => Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Can't understand request".to_string(),
                format!("Need to specify a '{}'", key),
            )),
        }
    }

    /// Get and remove the key specified from the arguments as a u64
    fn get_u64(
        &self,
        key: &str,
        args: &mut HashMap<String, serde_json::Value>,
        id: u64,
    ) -> Result<u64> {
        match args.remove(key) {
            Some(k) => match k.clone() {
                serde_json::Value::Number(n) => match n.as_u64() {
                    Some(i) => Ok(i),
                    None => Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        id,
                        format!("Badly specified 64-bit unsigned integer '{}'", key),
                        format!("Badly specified 64-bit unsigned integer '{}': {}", key, &k),
                    )),
                },
                _ => Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    format!("Badly specified 64-bit unsigned integer '{}'", key),
                    format!("Badly specified 64-bit unsigned integer '{}': {}", key, &k),
                )),
            },
            None => Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Can't understand request".to_string(),
                format!("Need to specify a '{}'", key),
            )),
        }
    }
}

impl<'a> Decoder for VimCodec<'a> {
    type Item = PadreRequest;
    type Error = PadreErrorWithId;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
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

                    src.advance(src.len());

                    return Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        0,
                        "Must be valid JSON".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    ));
                }
            },
            None => {
                return Ok(None);
            }
        };

        let offset = stream.byte_offset();
        src.advance(offset);

        if !v.is_array() {
            return Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                0,
                "Not an array, invalid JSON".to_string(),
                format!(
                    "Can't read '{}': Must be an array",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            ));
        }

        if v.as_array().unwrap().len() == 0 {
            return Err(PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                0,
                "Array must have 2 elements, invalid JSON".to_string(),
                format!(
                    "Can't read '{}': Array should have 2 elements",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            ));
        }

        let id = v[0].take();
        let id: u64 = match serde_json::from_value(id.clone()) {
            Ok(s) => s,
            Err(e) => {
                return Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    0,
                    "Can't read id".to_string(),
                    format!("Can't read '{}': {}", id, e),
                ));
            }
        };

        if v.as_array().unwrap().len() != 2 {
            let e = PadreErrorWithId::new(
                PadreErrorKind::RequestSyntaxError,
                id,
                "Array must have 2 elements, invalid JSON".to_string(),
                format!(
                    "Can't read '{}': Array should have 2 elements",
                    String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                ),
            );
            return Err(e);
        }

        let mut args: HashMap<String, serde_json::Value> =
            match serde_json::from_str(&v[1].take().to_string()) {
                Ok(args) => args,
                Err(e) => {
                    return Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        id,
                        "Can't read 2nd argument as dictionary".to_string(),
                        format!(
                            "Can't read '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    ));
                }
            };

        let cmd: String = match args.remove("cmd") {
            Some(s) => match serde_json::from_value(s) {
                Ok(s) => s,
                Err(e) => {
                    return Err(PadreErrorWithId::new(
                        PadreErrorKind::RequestSyntaxError,
                        id,
                        "Can't find command".to_string(),
                        format!(
                            "Can't find command '{}': {}",
                            String::from_utf8_lossy(&req[..]).trim_matches(char::from(0)),
                            e
                        ),
                    ));
                }
            },
            None => {
                return Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    "Can't find command".to_string(),
                    format!(
                        "Can't find command '{}': Need a cmd in 2nd object",
                        String::from_utf8_lossy(&req[..]).trim_matches(char::from(0))
                    ),
                ));
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
                RequestCmd::DebuggerCmd(DebuggerCmd::Run),
            ))),
            "stepOver" => {
                let count = match self.get_u64("count", &mut args, id) {
                    Ok(c) => c,
                    Err(_) => 1,
                };
                Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::DebuggerCmd(DebuggerCmd::StepOver(count)),
                )))
            }
            "stepIn" => {
                let count = match self.get_u64("count", &mut args, id) {
                    Ok(c) => c,
                    Err(_) => 1,
                };
                Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::DebuggerCmd(DebuggerCmd::StepIn(count)),
                )))
            }
            "continue" => Ok(Some(PadreRequest::new(
                id,
                RequestCmd::DebuggerCmd(DebuggerCmd::Continue),
            ))),
            "breakpoint" => {
                let file_location = self.get_file_location(&mut args, id);
                match file_location {
                    Ok(fl) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::DebuggerCmd(DebuggerCmd::Breakpoint(fl)),
                    ))),
                    Err(e) => return Err(e),
                }
            }
            "unbreakpoint" => {
                let file_location = self.get_file_location(&mut args, id);
                match file_location {
                    Ok(fl) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::DebuggerCmd(DebuggerCmd::Unbreakpoint(fl)),
                    ))),
                    Err(e) => return Err(e),
                }
            }
            "print" => {
                let variable = self.get_variable(&mut args, id);
                match variable {
                    Ok(v) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::DebuggerCmd(DebuggerCmd::Print(v)),
                    ))),
                    Err(e) => return Err(e),
                }
            }
            "getConfig" => {
                let key = self.get_string("key", &mut args, id);
                match key {
                    Ok(k) => Ok(Some(PadreRequest::new(
                        id,
                        RequestCmd::PadreCmd(PadreCmd::GetConfig(k)),
                    ))),
                    Err(e) => return Err(e),
                }
            }
            "setConfig" => {
                let key = self.get_string("key", &mut args, id);
                match key {
                    Ok(k) => {
                        let value = self.get_i64("value", &mut args, id);
                        match value {
                            Ok(v) => Ok(Some(PadreRequest::new(
                                id,
                                RequestCmd::PadreCmd(PadreCmd::SetConfig(k, v)),
                            ))),
                            Err(e) => return Err(e),
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
            _ => {
                return Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    "Command unknown".to_string(),
                    format!("Command unknown: '{}'", cmd),
                ));
            }
        };

        match args.is_empty() {
            true => {}
            false => {
                let mut args_left: Vec<String> = args.iter().map(|(key, _)| key.clone()).collect();
                args_left.sort();
                return Err(PadreErrorWithId::new(
                    PadreErrorKind::RequestSyntaxError,
                    id,
                    "Bad arguments".to_string(),
                    format!("Bad arguments: {:?}", args_left),
                ));
            }
        };

        ret
    }
}

impl<'a> Encoder<PadreSend> for VimCodec<'a> {
    type Error = PadreError;

    fn encode(&mut self, resp: PadreSend, buf: &mut BytesMut) -> padre_core::Result<()> {
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
        buf.put(response[..].as_bytes());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::config::Config;
    use crate::server::{PadreCmd, PadreResponse, PadreSend, RequestCmd};
    use padre_core::debugger::DebuggerCmd;
    use padre_core::server::Notification;

    use bytes::{BufMut, BytesMut};
    use tokio_util::codec::{Decoder, Encoder};

    #[test]
    fn check_simple_json_decoding() {
        let config = Arc::new(Mutex::new(Config::new()));

        let mut codec = super::VimCodec::new(config);
        let mut buf = BytesMut::new();
        buf.reserve(19);
        let s = r#"[123,{"cmd":"run"}]"#;
        buf.put(s.as_bytes());

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(123, padre_request.id());

        match padre_request.cmd() {
            RequestCmd::DebuggerCmd(cmd) => {
                assert_eq!(DebuggerCmd::Run, *cmd);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn check_two_simple_json_decoding() {
        let config = Arc::new(Mutex::new(Config::new()));

        let mut codec = super::VimCodec::new(config);
        let mut buf = BytesMut::new();
        buf.reserve(19);
        let s = r#"[123,{"cmd":"run"}]"#;
        buf.put(s.as_bytes());

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(123, padre_request.id());

        match padre_request.cmd() {
            RequestCmd::DebuggerCmd(cmd) => {
                assert_eq!(DebuggerCmd::Run, *cmd);
            }
            _ => panic!("Wrong command type"),
        }

        let mut buf = BytesMut::new();
        buf.reserve(20);
        let s = r#"[124,{"cmd":"ping"}]"#;
        buf.put(s.as_bytes());

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(124, padre_request.id());

        match padre_request.cmd() {
            RequestCmd::PadreCmd(cmd) => {
                assert_eq!(PadreCmd::Ping, *cmd);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn check_two_buffers_json_decodings() {
        let config = Arc::new(Mutex::new(Config::new()));

        let mut codec = super::VimCodec::new(config);
        let mut buf = BytesMut::new();
        buf.reserve(16);
        let s = r#"[123,{"cmd":"run"#;
        buf.put(s.as_bytes());

        let padre_request = codec.decode(&mut buf).unwrap();

        assert_eq!(None, padre_request);

        buf.reserve(3);
        let s = r#""}]"#;
        buf.put(s.as_bytes());

        let padre_request = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(123, padre_request.id());

        match padre_request.cmd() {
            RequestCmd::DebuggerCmd(cmd) => {
                assert_eq!(DebuggerCmd::Run, *cmd);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn check_json_encoding_response() {
        let config = Arc::new(Mutex::new(Config::new()));

        let mut codec = super::VimCodec::new(config);
        let resp = PadreSend::Response(PadreResponse::new(123, serde_json::json!({"ping":"pong"})));
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf).unwrap();

        let mut expected = BytesMut::new();
        expected.reserve(22);
        let s = format!("{}{}", r#"[123,{"ping":"pong"}]"#, "\n");
        expected.put(s.as_bytes());

        assert_eq!(expected, buf);
    }

    #[test]
    fn check_json_encoding_notify() {
        let config = Arc::new(Mutex::new(Config::new()));

        let mut codec = super::VimCodec::new(config);
        let resp = PadreSend::Notification(Notification::new(
            "cmd_test".to_string(),
            vec![serde_json::json!("test"), serde_json::json!(1)],
        ));
        let mut buf = BytesMut::new();
        codec.encode(resp, &mut buf).unwrap();

        let mut expected = BytesMut::new();
        expected.reserve(31);
        let s = format!("{}{}", r#"["call","cmd_test",["test",1]]"#, "\n");
        expected.put(s.as_bytes());

        assert_eq!(expected, buf);
    }
}
