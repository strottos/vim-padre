//! handling requests

use std::io::{Result, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use crate::debugger::PadreServer;

pub enum Response<T> {
    OK(T),
    PENDING(T)
}

// TODO: Send back logs instead of expect and panic when structure in place.
pub fn handle_connection(mut stream: TcpStream, padre_server: Arc<Mutex<PadreServer>>) {
    loop {
        let mut buffer = [0; 512];

        stream.read(&mut buffer).expect("Can't read from socket!");

        let data = String::from_utf8_lossy(&buffer[..]);

        let json = json::parse(data.trim()
                        .trim_matches(char::from(0)))
                        .expect("Must be valid JSON");

        let id = json[0].as_u32().expect("Can't find ID");
        let cmd = json[1].as_str().expect("Can't find cmd");

        let ret = handle_cmd(cmd, &padre_server).expect(
            &format!("Can't perform cmd {}", cmd));
        let response = match ret {
            Response::OK(s) => {
                match s {
                    Some(t) => format!("OK {}", t),
                    None => String::from("OK"),
                }
            }
            Response::PENDING(s) => {
                match s {
                    Some(t) => format!("PENDING {}", t),
                    None => String::from("PENDING"),
                }
            }
        };

        stream.write(&format!("[{},\"{}\"]", id, response).into_bytes())
              .expect("Can't write to socket");
    }
}

fn handle_cmd(cmd: &str, padre_server: &Arc<Mutex<PadreServer>>) -> Result<Response<Option<String>>> {
    match cmd.to_string().split_whitespace().nth(0) {
        Some(s) => {
            // TODO: Find better method than unwrap()
            match s {
                "ping" => padre_server.lock().unwrap().ping(),
                "pings" => padre_server.lock().unwrap().pings(),
                _ => Ok(Response::OK(Some(String::from(cmd))))
            }
        },
        None => panic!("Can't understand string"),
    }
}
