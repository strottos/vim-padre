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
