//! VIMCodec
//!
//! Rust Tokio Codec for communicating with VIM

use std::collections::HashMap;
use std::io;

use crate::server::{DebuggerCmd, PadreCmd, PadreRequest, PadreSend, RequestCmd};

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

        let mut v = match stream.next() {
            Some(s) => match s {
                Ok(t) => t,
                Err(e) => {
                    return Ok(None);
                }
            },
            None => {
                unreachable!();
            }
        };

        src.split_to(src.len());

        let id = v[0].take();
        let id: u64 = match serde_json::from_value(id.clone()) {
            Ok(s) => s,
            Err(e) => {
                return Ok(None);
            }
        };

        let mut args: HashMap<String, serde_json::Value> =
            match serde_json::from_str(&v[1].take().to_string()) {
                Ok(args) => args,
                Err(e) => {
                    return Ok(None);
                }
            };

        let cmd: String = match args.remove("cmd") {
            Some(s) => match serde_json::from_value(s) {
                Ok(s) => s,
                Err(e) => return Ok(None),
            },
            None => {
                return Ok(None);
            }
        };

        match &cmd[..] {
            "ping" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestPadreCmd(PadreCmd::Ping),
                )))
            }
            "pings" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestPadreCmd(PadreCmd::Pings),
                )))
            }
            "run" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestDebuggerCmd(DebuggerCmd::Run),
                )))
            }
            "stepOver" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestDebuggerCmd(DebuggerCmd::StepOver),
                )))
            }
            "stepIn" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestDebuggerCmd(DebuggerCmd::StepIn),
                )))
            }
            "continue" => {
                return Ok(Some(PadreRequest::new(
                    id,
                    RequestCmd::RequestDebuggerCmd(DebuggerCmd::Continue),
                )))
            }
            _ => {}
        };

        return Ok(None);
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
    use crate::server::{
        DebuggerCmd, Notification, PadreCmd, PadreRequest, PadreSend, RequestCmd, Response,
    };

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
            PadreRequest::new(123, RequestCmd::RequestDebuggerCmd(DebuggerCmd::Run)),
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
            PadreRequest::new(123, RequestCmd::RequestDebuggerCmd(DebuggerCmd::Run)),
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
            PadreRequest::new(123, RequestCmd::RequestDebuggerCmd(DebuggerCmd::Run)),
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
