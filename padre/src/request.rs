//! handling requests

use std::error::Error;
use std::fmt;
use std::io;
use std::io::{Read, Write};
use std::result::Result;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreServer;
use crate::notifier::{LogLevel, Notifier};

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
    fn new(msg: String, debug: String) -> RequestError {
        RequestError {
            msg: msg,
            debug: debug,
        }
    }

    fn get_debug_info(&self) -> &str {
        &self.debug
    }
}

// TODO: Send back logs instead of expect and panic when structure in place.
pub fn handle_connection(mut stream: TcpStream, notifier: Arc<Mutex<Notifier>>, padre_server: Arc<Mutex<PadreServer>>) {
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

        let ret = handle_cmd(&cmd, &padre_server);
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

        stream.write(&format!("[{},\"{}\"]", id, response).into_bytes())
              .expect("Can't write to socket");
    }
}

fn handle_cmd(cmd: &str, padre_server: &Arc<Mutex<PadreServer>>) -> Result<Response<Option<String>>, RequestError> {
    match cmd.to_string().split_whitespace().nth(0) {
        Some(s) => {
            match s {
                // TODO: Find better method than unwrap()
                "ping" => padre_server.lock().unwrap().ping(),
                "pings" => padre_server.lock().unwrap().pings(),
                _ => Err(RequestError::new("Can't understand request".to_string(),
                                           format!("Can't understand request: {}", cmd)))
            }
        },
        None => panic!("Can't understand string"),
    }
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
                                                 format!("Can't read id from: {}",
                                                         data.to_string().replace("\"", "\\\""))))
                }
            };

            let cmd = match s[1].as_str() {
                Some(t) => t.to_string(),
                None => {
                    return Err(RequestError::new("Can't read command".to_string(),
                                                 format!("Can't read command from: {}",
                                                         data.to_string().replace("\"", "\\\""))))
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
                                                   data.to_string().replace("\"", "\\\"")))),

                // TODO: Cover these, they're unusual I believe
                _ => panic!(format!("Can't recover from error: {}", err)),
            }
        }
    }
}

fn handle_error(notifier: &Arc<Mutex<Notifier>>, err: RequestError) {
    notifier.lock()
            .unwrap()
            .log_msg(LogLevel::ERROR, format!("{}", err));
    notifier.lock()
            .unwrap()
            .log_msg(LogLevel::DEBUG, format!("{}", err.get_debug_info()));
}
