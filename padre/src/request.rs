//! handling requests

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::result::Result;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreServer;
use crate::notifier::{LogLevel, Notifier};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    fn check_error(err: super::RequestError, msg: &str, debug: &str) {
        assert_eq!(format!("{}", err), msg.to_string());
        assert_eq!(err.get_debug_info(), debug.to_string());
    }

    #[test]
    fn check_json_good_request_handled() {
        let ret = super::handle_json("[1,\"ping\"]");
        assert_eq!(ret.is_ok(), true);
        assert_eq!(ret.unwrap(), (1, "ping".to_string()));
    }

    #[test]
    fn check_json_nonsense_handled() {
        let ret = super::handle_json("nonsense");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Must be valid JSON",
                    "Can't read JSON character o in line 1 at column 2: nonsense");
    }

    #[test]
    fn check_json_no_end_handled() {
        let ret = super::handle_json("[1,\"no end\"");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Must be valid JSON",
                    "Can't read JSON: [1,\"no end\"");
    }

    #[test]
    fn check_json_bad_id_handled() {
        let ret = super::handle_json("[\"a\",\"b\"]");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Can't read id",
                    "Can't read id: [\"a\",\"b\"]");
    }

    #[test]
    fn check_json_bad_cmd_handled() {
        let ret = super::handle_json("[1,2]");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Can't read command",
                    "Can't read command: [1,2]");
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
        check_error(ret.err().unwrap(), "Can't find command",
                    "Can't find command: \"\"");
    }

    #[test]
    fn check_cmd_cmd_with_bad_arg_handled() {
        let ret = super::interpret_cmd("breakpoint test");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Can't understand arguments",
                    "Can't understand arguments: [\"test\"]");

        let ret = super::interpret_cmd("breakpoint test=test=test");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Can't understand arguments",
                    "Can't understand arguments: [\"test=test=test\"]");
    }

    #[test]
    fn check_cmd_cmd_with_bad_args_handled() {
        let ret = super::interpret_cmd("breakpoint test 1");
        assert_eq!(ret.is_err(), true);
        check_error(ret.err().unwrap(), "Can't understand arguments",
                   "Can't understand arguments: [\"test\",\"1\"]");
    }
}

pub enum Response<T> {
    OK(T),
    PENDING(T)
}

#[derive(Debug)]
pub struct RequestError {
    msg: String,
    debug: String,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}", self.msg)
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

// TODO: Work out how to handle networking errors
pub fn handle_connection(mut stream: TcpStream, notifier: Arc<Mutex<Notifier>>, padre_server: Arc<Mutex<PadreServer>>) {
    if padre_server.lock().unwrap().debugger.lock().unwrap().has_started() {
        let msg = "[\"call\",\"padre#debugger#SignalPADREStarted\",[]]".to_string();
        stream.write(msg.as_bytes())
              .expect("Can't write to socket");
    }

    loop {
        let mut buffer = [0; 512];

        stream.read(&mut buffer).expect("Can't read from socket!");

        let data = String::from_utf8_lossy(&buffer[..]);

        let (id, cmd) = match handle_json(&data) {
            Ok(s) => s,
            Err(err) => {
                handle_error(&notifier, err);
                continue;
            }
        };

        let ret = handle_cmd(cmd.to_string(), &padre_server, &notifier);
        let response = match ret {
            Ok(s) => {
                match s {
                    Response::OK(t) => {
                        match t {
                            Some(u) => format!("OK {}", u),
                            None => String::from("OK"),
                        }
                    },
                    Response::PENDING(t) => {
                        match t {
                            Some(u) => format!("PENDING {}", u),
                            None => String::from("PENDING"),
                        }
                    }
                }
            },
            Err(err) => {
                handle_error(&notifier, err);
                stream.write(&format!("[{},\"ERROR\"]", id).into_bytes())
                      .expect("Can't write to socket");
                continue;
            }
        };

        stream.write(&format!("[{},{}]", id, json::stringify(response))
                     .into_bytes())
              .expect("Can't write to socket");
    }
}

fn handle_cmd(data: String, padre_server: &Arc<Mutex<PadreServer>>, notifier: &Arc<Mutex<Notifier>>) -> Result<Response<Option<String>>, RequestError> {
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
        "run" => padre_server.lock()
                             .unwrap()
                             .debugger
                             .lock()
                             .unwrap()
                             .run(),
        "breakpoint" => {
            let bad_args = get_bad_args(&args, vec!("file", "line"));

            if bad_args.len() != 0 {
                return Err(RequestError::new("Bad arguments for breakpoint".to_string(),
                                             format!("Bad arguments for breakpoint: {}", json::stringify(bad_args))));
            }

            let file = match args.get("file") {
                Some(s) => s.to_string(),
                None => return Err(
                    RequestError::new("Can't read file for breakpoint".to_string(),
                                      "Can't read file for breakpoint".to_string()))
            };

            let line = match args.get("line") {
                Some(s) => s.to_string(),
                None => return Err(
                    RequestError::new("Can't read line for breakpoint".to_string(),
                                      "Can't read line for breakpoint".to_string()))
            };

            let line = match line.parse::<u32>() {
                Ok(s) => s,
                Err(err) => return Err(
                    RequestError::new("Can't parse line number".to_string(),
                                      format!("Can't parse line number: {}", err)))
            };

            notifier.lock().unwrap().log_msg(LogLevel::INFO,
                format!("Setting breakpoint in file {} at line number {}", file, line));

            padre_server.lock()
                        .unwrap()
                        .debugger
                        .lock()
                        .unwrap()
                        .breakpoint(file, line)
        },
        "stepIn" => padre_server.lock()
                                .unwrap()
                                .debugger
                                .lock()
                                .unwrap()
                                .stepIn(),
        "stepOver" => padre_server.lock()
                                  .unwrap()
                                  .debugger
                                  .lock()
                                  .unwrap()
                                  .stepOver(),
        "continue" => padre_server.lock()
                                  .unwrap()
                                  .debugger
                                  .lock()
                                  .unwrap()
                                  .carryOn(),
        "print" => {
            let bad_args = get_bad_args(&args, vec!("variable"));

            if bad_args.len() != 0 {
                return Err(RequestError::new("Bad arguments for print".to_string(),
                                             format!("Bad arguments for print: {}", json::stringify(bad_args))));
            }

            let variable = match args.get("variable") {
                Some(s) => s.to_string(),
                None => return Err(
                    RequestError::new("Can't read variable for print".to_string(),
                                      "Can't read variable for print".to_string()))
            };

            padre_server.lock()
                        .unwrap()
                        .debugger
                        .lock()
                        .unwrap()
                        .print(variable)
        },
        _ => Err(RequestError::new("Can't understand request".to_string(),
                                   format!("Can't understand request: {}", data)))
    }
}

fn interpret_cmd(data: &str) -> Result<(String, HashMap<String, String>), RequestError> {
    let mut args = HashMap::new();

    let mut data_split = data.split_whitespace();

    let cmd = match &data_split.nth(0) {
        Some(s) => s.to_string(),
        None => {
            return Err(RequestError::new("Can't find command".to_string(),
                                         format!("Can't find command: \"{}\"", data)));
        }
    };

    let mut bad_args = vec!();

    for arg in data_split.skip(0) {
        let mut arg_tuple = arg.split("=");

        let key = match arg_tuple.next() {
            Some(s) => s,
            None => {
                bad_args.push(arg);
                ""
            }
        }.to_string();
        let value = match arg_tuple.next() {
            Some(s) => s,
            None => {
                bad_args.push(arg);
                ""
            }
        }.to_string();
        match arg_tuple.next() {
            Some(s) => {
                bad_args.push(arg);
            },
            None => ()
        }
        args.insert(key, value);
    }

    if bad_args.len() != 0 {
        return Err(RequestError::new("Can\'t understand arguments".to_string(),
                                     format!("Can't understand arguments: {}", json::stringify(bad_args))));
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
                    return Err(RequestError::new("Can't read id".to_string(),
                                                 format!("Can't read id: {}",
                                                         data.to_string())))
                }
            };

            let cmd = match s[1].as_str() {
                Some(t) => t.to_string(),
                None => {
                    return Err(RequestError::new("Can't read command".to_string(),
                                                 format!("Can't read command: {}",
                                                         data.to_string())))
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
                } => Err(RequestError::new("Must be valid JSON".to_string(),
                                           format!("Can't read JSON character {} in line {} at column {}: {}",
                                                   ch, line, column, data))),

                json::Error:: UnexpectedEndOfJson =>
                    Err(RequestError::new("Must be valid JSON".to_string(),
                                           format!("Can't read JSON: {}",
                                                   data.to_string()))),

                // TODO: Cover these, they're unusual I believe
                _ => panic!(format!("Can't recover from error: {}", err)),
            }
        }
    }
}

fn get_bad_args(args: &HashMap<String, String>, valid_args: Vec<&str>) -> Vec<String> {
    let mut bad_args = vec!();

    for key in args.keys() {
        if !valid_args.contains(&key.as_str()) {
            bad_args.push(format!("{}", key));
        }
    }

    bad_args.sort_unstable();

    bad_args
}

fn handle_error(notifier: &Arc<Mutex<Notifier>>, err: RequestError) {
    notifier.lock()
            .unwrap()
            .log_msg(LogLevel::ERROR, format!("{}", err));
    notifier.lock()
            .unwrap()
            .log_msg(LogLevel::DEBUG, format!("{}", err.get_debug_info()));
}
