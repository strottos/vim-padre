//! handling requests

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::result::Result;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreDebugger;
use crate::notifier::{LogLevel, Notifier};

use bytes::{BufMut, BytesMut};
use futures::sync::mpsc;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    fn check_error(err: super::RequestError, msg: &str, debug: &str) {
        assert_eq!(format!("{}", err), msg.to_string());
        assert_eq!(err.get_debug_info(), debug.to_string());
    }

    #[test]
    fn check_json_good_request_handled() {
        let ret = super::handle_json("[1,{\"cmd\",\"ping\"}]");
        assert_eq!(ret.is_ok(), true);
        assert_eq!(ret.unwrap(), (1, "ping".to_string()));
    }

    #[test]
    fn check_json_nonsense_handled() {
        let ret = super::handle_json("nonsense");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Must be valid JSON",
            "Can't read JSON character o in line 1 at column 2: nonsense",
        );
    }

    #[test]
    fn check_json_no_end_handled() {
        let ret = super::handle_json("[1,\"no end\"");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Must be valid JSON",
            "Can't read JSON: [1,\"no end\"",
        );
    }

    #[test]
    fn check_json_bad_id_handled() {
        let ret = super::handle_json("[\"a\",\"b\"]");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't read id",
            "Can't read id: [\"a\",\"b\"]",
        );
    }

    #[test]
    fn check_json_bad_cmd_handled() {
        let ret = super::handle_json("[1,{}]");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't read command",
            "Can't read command: [1,2]",
        );
    }

    #[test]
    fn check_cmd_single_cmd_handled() {
        let ret = super::interpret_cmd("stepIn");
        let expected_args = HashMap::new();
        assert_eq!(ret.is_ok(), true);
        assert_eq!(ret.unwrap(), ("stepIn".to_string(), expected_args));
    }

    #[test]
    fn check_cmd_cmd_with_args_handled() {
        let ret = super::interpret_cmd("breakpoint file=test.c line=1");
        let mut expected_args = HashMap::new();
        expected_args.insert("file".to_string(), "test.c".to_string());
        expected_args.insert("line".to_string(), "1".to_string());
        assert_eq!(ret.is_ok(), true);
        assert_eq!(ret.unwrap(), ("breakpoint".to_string(), expected_args));
    }

    #[test]
    fn check_cmd_no_cmd_handled() {
        let ret = super::interpret_cmd("");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't find command",
            "Can't find command: \"\"",
        );
    }

    #[test]
    fn check_cmd_cmd_with_bad_arg_handled() {
        let ret = super::interpret_cmd("breakpoint test");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't understand arguments",
            "Can't understand arguments: [\"test\"]",
        );

        let ret = super::interpret_cmd("breakpoint test=test=test");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't understand arguments",
            "Can't understand arguments: [\"test=test=test\"]",
        );
    }

    #[test]
    fn check_cmd_cmd_with_bad_args_handled() {
        let ret = super::interpret_cmd("breakpoint test 1");
        assert_eq!(ret.is_err(), true);
        check_error(
            ret.err().unwrap(),
            "Can't understand arguments",
            "Can't understand arguments: [\"test\",\"1\"]",
        );
    }
}

#[derive(Debug)]
pub enum Response<T> {
    OK(T),
    PENDING(T),
}

#[derive(Debug)]
pub struct RequestError {
    msg: String,
    debug: String,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for RequestError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl RequestError {
    pub fn new(msg: String, debug: String) -> RequestError {
        RequestError {
            msg: msg,
            debug: debug,
        }
    }

    fn get_debug_info(&self) -> &str {
        &self.debug
    }
}

#[derive(Debug)]
pub struct PadreRequest {
    id: u32,
    cmd: String,
    args: HashMap<String, String>,
    padre_server: Arc<Mutex<PadreDebugger>>,
    response: Option<json::object::Object>,
}

impl PadreRequest {
    pub fn new(
        id: u32,
        cmd: String,
        args: HashMap<String, String>,
        padre_server: Arc<Mutex<PadreDebugger>>,
    ) -> Self {
        PadreRequest {
            id,
            cmd,
            args,
            padre_server,
            response: None,
        }
    }
}

impl fmt::Display for PadreRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{},{{\"cmd\":\"{}\",\"args\":{:?}}}]",
            self.id, self.cmd, self.args
        )
    }
}

impl Future for PadreRequest {
    type Item = Response<json::object::Object>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        println!("HERE Running Padre Request Future");

        let response = match self.cmd.as_str() {
            // TODO: Find better method than unwrap()
            "ping" => self.padre_server.lock().unwrap().ping(),
            "pings" => self.padre_server.lock().unwrap().pings(),
            "run" => self
                .padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .run(),
            //            "breakpoint" => {
            //                let bad_args = get_bad_args(&args, vec!("file", "line"));
            //
            //                if bad_args.len() != 0 {
            //                    return Err(RequestError::new("Bad arguments for breakpoint".to_string(),
            //                                                 format!("Bad arguments for breakpoint: {}", json::stringify(bad_args))));
            //                }
            //
            //                let file = match args.get("file") {
            //                    Some(s) => s.to_string(),
            //                    None => return Err(
            //                        RequestError::new("Can't read file for breakpoint".to_string(),
            //                                          "Can't read file for breakpoint".to_string()))
            //                };
            //
            //                let line = match args.get("line") {
            //                    Some(s) => s.to_string(),
            //                    None => return Err(
            //                        RequestError::new("Can't read line for breakpoint".to_string(),
            //                                          "Can't read line for breakpoint".to_string()))
            //                };
            //
            //                let line = match line.parse::<u32>() {
            //                    Ok(s) => s,
            //                    Err(err) => return Err(
            //                        RequestError::new("Can't parse line number".to_string(),
            //                                          format!("Can't parse line number: {}", err)))
            //                };
            //
            //                notifier.lock().unwrap().log_msg(LogLevel::INFO,
            //                    format!("Setting breakpoint in file {} at line number {}", file, line));
            //
            //                padre_server.lock()
            //                            .unwrap()
            //                            .debugger
            //                            .lock()
            //                            .unwrap()
            //                            .breakpoint(file, line)
            //            },
            "stepIn" => self
                .padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .step_in(),
            "stepOver" => self
                .padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .step_over(),
            "continue" => self
                .padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .continue_on(),
            //            "print" => {
            //                let bad_args = get_bad_args(&args, vec!("variable"));
            //
            //                if bad_args.len() != 0 {
            //                    return Err(RequestError::new("Bad arguments for print".to_string(),
            //                                                 format!("Bad arguments for print: {}", json::stringify(bad_args))));
            //                }
            //
            //                let variable = match args.get("variable") {
            //                    Some(s) => s.to_string(),
            //                    None => return Err(
            //                        RequestError::new("Can't read variable for print".to_string(),
            //                                          "Can't read variable for print".to_string()))
            //                };
            //
            //                padre_server.lock()
            //                            .unwrap()
            //                            .debugger
            //                            .lock()
            //                            .unwrap()
            //                            .print(variable)
            //            },
            _ => Err(RequestError::new(
                "Can't understand request".to_string(),
                format!("Can't understand request: {}", self),
            )),
        };

        println!("Response: {:?}", response);

        Ok(Async::Ready(response.unwrap())) // TODO: Error handle response
    }
}

#[derive(Debug)]
struct PadreCodec {}

//impl Encoder for PadreCodec {
//    type Item = PadreRequest;
//    type Error = io::Error;
//
//    fn encode(&mut self, padre_cmd: PadreRequest, buf: &mut BytesMut) -> Result<(), io::Error> {
//        println!("ENCODING");
//        let response = match padre_cmd.response {
//            Some(s) => s,
//            None => json::object::Object::new(),
//        };
//
//        let response = json::stringify(json::array![json::from(padre_cmd.id), response]);
//
//        buf.reserve(response.len());
//        buf.put(&response[..]);
//
//        Ok(())
//    }
//}

//struct PadreWriter {
//    writer: WriteHalf<TcpStream>,
//    rx: mpsc::UnboundedReceiver<String>,
//}
//
//impl Future for PadreWriter {
//    type Item = ();
//    type Error = io::Error;
//
//    fn poll(&mut self) -> Poll<(), io::Error> {
//        loop {
//            match self.rx.poll().unwrap() {
//                Async::Ready(Some(v)) => {
//                    println!("{:?}", v);
//                    return Ok(Async::Ready(()));
//                }
//                _ => break,
//            }
//        }
//
//        Ok(Async::NotReady)
//    }
//}

#[derive(Debug)]
pub struct PadreConnection {
    addr: SocketAddr,
    reader: ReadHalf<TcpStream>,
    notifier: Arc<Mutex<Notifier>>,
    padre_server: Arc<Mutex<PadreDebugger>>,
    rd: BytesMut,
}

impl PadreConnection {
    pub fn new(
        socket: TcpStream,
        notifier: Arc<Mutex<Notifier>>,
        padre_server: Arc<Mutex<PadreDebugger>>,
    ) -> Self {
        let addr = socket.peer_addr().unwrap();

        let (reader, writer) = socket.split();

        //        let transport_read = FramedRead::new(reader, PadreCodec{});
        //
        //        tokio::spawn({
        //            transport_read.for_each(|padre_cmd| {
        //                println!("GOT: {:?}", padre_cmd);
        //
        //                Ok(())
        //            }).map_err(|err| {
        //                println!("Error: {}", err);
        //            })
        //        });

        notifier.lock().unwrap().add_listener(writer, addr);

        PadreConnection {
            addr,
            reader,
            notifier,
            padre_server,
            rd: BytesMut::new(),
        }
    }

    // Based on from https://github.com/tokio-rs/tokio/blob/master/tokio/examples/chat.rs
    // Returns ready only when the socket is closed
    fn fill_read_buf(&mut self) -> Poll<(), io::Error> {
        loop {
            self.rd.reserve(1024);

            let n = try_ready!(self.reader.read_buf(&mut self.rd));

            if n == 0 {
                return Ok(Async::Ready(()));
            }
        }
    }

    fn decode(&mut self) -> Result<Option<PadreRequest>, io::Error> {
        let data = String::from_utf8_lossy(&self.rd);
        let data = data.trim().trim_matches(char::from(0));

        let mut s = match json::parse(&data) {
            Ok(t) => t,
            Err(err) => return Ok(None), // TODO
        };

        //        match json {
        //            Ok(s) => {
        let id = s[0].as_u32().unwrap();
        //                match s[0].as_u32() {
        //                    Some(t) => t,
        //                    None => {
        //                        return Err(RequestError::new("Can't read id".to_string(),
        //                                                     format!("Can't read id: {}",
        //                                                             data.to_string())))
        //                    }
        //                };

        let mut args = HashMap::new();
        let mut cmd: String = "".to_string();

        let s = s[1].take();
        for entry in s.entries() {
            let key = entry.0.to_string();
            let value = entry.1.as_str().unwrap().to_string();
            if key == "cmd" {
                cmd = value;
                continue;
            }
            args.insert(key, value);
        }

        self.rd.split_to(self.rd.len());

        Ok(Some(PadreRequest::new(
            id,
            cmd,
            args,
            Arc::clone(&self.padre_server), // TODO: More efficient way??
        )))
        //            },
        //            Err(err) => {
        //                match err {
        //                    json::Error::UnexpectedCharacter {
        //                        ref ch,
        //                        ref line,
        //                        ref column,
        //                    } => Err(RequestError::new("Must be valid JSON".to_string(),
        //                                               format!("Can't read JSON character {} in line {} at column {}: {}",
        //                                                       ch, line, column, data))),
        //
        //                    json::Error:: UnexpectedEndOfJson =>
        //                        Err(RequestError::new("Must be valid JSON".to_string(),
        //                                               format!("Can't read JSON: {}",
        //                                                       data.to_string()))),
        //
        //                    // TODO: Cover these, they're unusual I believe
        //                    _ => panic!(format!("Can't recover from error: {}", err)),
        //                }
        //            }
        //        };
    }
}

impl Drop for PadreConnection {
    fn drop(&mut self) {
        self.notifier.lock().unwrap().remove_listener(&self.addr);
    }
}

impl Stream for PadreConnection {
    type Item = PadreRequest;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let sock_closed = self.fill_read_buf()?.is_ready();

        let padre_request = self.decode();
        match padre_request {
            Ok(s) => match s {
                Some(mut t) => {
                    match t.poll() {
                        Ok(u) => {
                            println!("Polling Padre Request Future OK: {:?}", u);
                            return Ok(Async::Ready(Some(t)));
                        }
                        _ => panic!("TODO: Figure out"),
                    }
                }
                None => {}
            },
            Err(err) => {
                println!("TODO: Handle Error {:?}:", err);
                panic!("ERROR");
            }
        };

        if sock_closed {
            Ok(Async::Ready(None))
        } else {
            Ok(Async::NotReady)
        }
    }
}

// TODO: Work out how to handle networking errors
pub fn handle_connection(
    mut stream: TcpStream,
    notifier: Arc<Mutex<Notifier>>,
    padre_server: Arc<Mutex<PadreDebugger>>,
) {
    //    TODO
    //    if padre_server.lock().unwrap().process.lock().unwrap().has_started() {
    //        notifier.lock().unwrap().signal_started();
    //    }

    loop {
        let mut buffer = [0; 512];

        match stream.read(&mut buffer) {
            Ok(_) => {}
            Err(err) => {
                println!("Can't read from socket: {}", err);
                return;
            }
        };

        let data = String::from_utf8_lossy(&buffer[..]);

        let (id, cmd) = match handle_json(&data) {
            Ok(s) => s,
            Err(err) => {
                handle_error(&notifier, err);
                continue;
            }
        };

        let mut response = json::object::Object::new();

        let args: json::object::Object = match handle_cmd(cmd.to_string(), &padre_server, &notifier)
        {
            Ok(s) => match s {
                Response::OK(t) => {
                    response.insert("status", json::from("OK".to_string()));
                    t
                }
                Response::PENDING(t) => {
                    response.insert("status", json::from("PENDING".to_string()));
                    t
                }
            },
            Err(err) => {
                handle_error(&notifier, err);
                response.insert("status", json::from("ERROR".to_string()));
                json::object::Object::new()
            }
        };

        match args.get("status") {
            Some(_) => panic!("Can't specify status in response"),
            None => {}
        };

        for arg in args.iter() {
            response.insert(arg.0, arg.1.clone());
        }

        let response = json::array![id, response];

        stream
            .write(&json::stringify(response).into_bytes())
            .expect("Can't write to socket");
    }
}

fn handle_cmd(
    data: String,
    padre_server: &Arc<Mutex<PadreDebugger>>,
    notifier: &Arc<Mutex<Notifier>>,
) -> Result<Response<json::object::Object>, RequestError> {
    let (prog, args) = match interpret_cmd(&data) {
        Ok(s) => s,
        Err(err) => {
            return Err(err);
        }
    };

    match prog.as_str() {
        // TODO: Find better method than unwrap()
        "ping" => padre_server.lock().unwrap().ping(),
        "pings" => padre_server.lock().unwrap().pings(),
        "run" => padre_server.lock().unwrap().debugger.lock().unwrap().run(),
        "breakpoint" => {
            let bad_args = get_bad_args(&args, vec!["file", "line"]);

            if bad_args.len() != 0 {
                return Err(RequestError::new(
                    "Bad arguments for breakpoint".to_string(),
                    format!(
                        "Bad arguments for breakpoint: {}",
                        json::stringify(bad_args)
                    ),
                ));
            }

            let file = match args.get("file") {
                Some(s) => s.to_string(),
                None => {
                    return Err(RequestError::new(
                        "Can't read file for breakpoint".to_string(),
                        "Can't read file for breakpoint".to_string(),
                    ))
                }
            };

            let line = match args.get("line") {
                Some(s) => s.to_string(),
                None => {
                    return Err(RequestError::new(
                        "Can't read line for breakpoint".to_string(),
                        "Can't read line for breakpoint".to_string(),
                    ))
                }
            };

            let line = match line.parse::<u32>() {
                Ok(s) => s,
                Err(err) => {
                    return Err(RequestError::new(
                        "Can't parse line number".to_string(),
                        format!("Can't parse line number: {}", err),
                    ))
                }
            };

            notifier.lock().unwrap().log_msg(
                LogLevel::INFO,
                format!(
                    "Setting breakpoint in file {} at line number {}",
                    file, line
                ),
            );

            padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .breakpoint(file, line)
        }
        "stepIn" => padre_server
            .lock()
            .unwrap()
            .debugger
            .lock()
            .unwrap()
            .step_in(),
        "stepOver" => padre_server
            .lock()
            .unwrap()
            .debugger
            .lock()
            .unwrap()
            .step_over(),
        "continue" => padre_server
            .lock()
            .unwrap()
            .debugger
            .lock()
            .unwrap()
            .continue_on(),
        "print" => {
            let bad_args = get_bad_args(&args, vec!["variable"]);

            if bad_args.len() != 0 {
                return Err(RequestError::new(
                    "Bad arguments for print".to_string(),
                    format!("Bad arguments for print: {}", json::stringify(bad_args)),
                ));
            }

            let variable = match args.get("variable") {
                Some(s) => s.to_string(),
                None => {
                    return Err(RequestError::new(
                        "Can't read variable for print".to_string(),
                        "Can't read variable for print".to_string(),
                    ))
                }
            };

            padre_server
                .lock()
                .unwrap()
                .debugger
                .lock()
                .unwrap()
                .print(variable)
        }
        _ => Err(RequestError::new(
            "Can't understand request".to_string(),
            format!("Can't understand request: {}", data),
        )),
    }
}

fn interpret_cmd(data: &str) -> Result<(String, HashMap<String, String>), RequestError> {
    let mut args = HashMap::new();

    let mut data_split = data.split_whitespace();

    let cmd = match &data_split.nth(0) {
        Some(s) => s.to_string(),
        None => {
            return Err(RequestError::new(
                "Can't find command".to_string(),
                format!("Can't find command: \"{}\"", data),
            ));
        }
    };

    let mut bad_args = vec![];

    for arg in data_split.skip(0) {
        let mut arg_tuple = arg.split("=");

        let key = match arg_tuple.next() {
            Some(s) => s,
            None => {
                bad_args.push(arg);
                ""
            }
        }
        .to_string();
        let value = match arg_tuple.next() {
            Some(s) => s,
            None => {
                bad_args.push(arg);
                ""
            }
        }
        .to_string();
        match arg_tuple.next() {
            Some(_) => {
                bad_args.push(arg);
            }
            None => (),
        }
        args.insert(key, value);
    }

    if bad_args.len() != 0 {
        return Err(RequestError::new(
            "Can\'t understand arguments".to_string(),
            format!("Can't understand arguments: {}", json::stringify(bad_args)),
        ));
    }

    Ok((cmd, args))
}

fn handle_json(data: &str) -> Result<(u32, String), RequestError> {
    let data = data.trim().trim_matches(char::from(0));

    let json = json::parse(data);

    match json {
        Ok(s) => {
            let id = match s[0].as_u32() {
                Some(t) => t,
                None => {
                    return Err(RequestError::new(
                        "Can't read id".to_string(),
                        format!("Can't read id: {}", data.to_string()),
                    ))
                }
            };

            let cmd = match s[1].as_str() {
                Some(t) => t.to_string(),
                None => {
                    return Err(RequestError::new(
                        "Can't read command".to_string(),
                        format!("Can't read command: {}", data.to_string()),
                    ))
                }
            };

            Ok((id, cmd))
        }
        Err(err) => {
            match err {
                json::Error::UnexpectedCharacter {
                    ref ch,
                    ref line,
                    ref column,
                } => Err(RequestError::new(
                    "Must be valid JSON".to_string(),
                    format!(
                        "Can't read JSON character {} in line {} at column {}: {}",
                        ch, line, column, data
                    ),
                )),

                json::Error::UnexpectedEndOfJson => Err(RequestError::new(
                    "Must be valid JSON".to_string(),
                    format!("Can't read JSON: {}", data.to_string()),
                )),

                // TODO: Cover these, they're unusual I believe
                _ => panic!(format!("Can't recover from error: {}", err)),
            }
        }
    }
}

fn get_bad_args(args: &HashMap<String, String>, valid_args: Vec<&str>) -> Vec<String> {
    let mut bad_args = vec![];

    for key in args.keys() {
        if !valid_args.contains(&key.as_str()) {
            bad_args.push(format!("{}", key));
        }
    }

    bad_args.sort_unstable();

    bad_args
}

// TODO: Don't forget to handle this, super important
fn handle_error(notifier: &Arc<Mutex<Notifier>>, err: RequestError) {
    let err_details = format!("{}", err);

    if err_details != "" {
        notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::ERROR, err_details);
    }

    let debug_info = format!("{}", err.get_debug_info());

    if debug_info != "" {
        notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::DEBUG, debug_info);
    }
}
