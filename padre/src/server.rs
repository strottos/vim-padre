//! handle server connections

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::debugger::DebugServer;
use crate::notifier::{LogLevel, Notifier};
use crate::vimcodec::VimCodec;

use tokio::codec::Decoder;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::sync::mpsc;

#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum RequestCmd {
    Cmd(String),
    CmdWithFileLocation(String, String, u64),
    CmdWithVariable(String, String),
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Request {
    id: u64,
    cmd: RequestCmd,
}

impl Request {
    pub fn new(id: u64, cmd: RequestCmd) -> Self {
        Request { id, cmd }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn cmd(&self) -> &RequestCmd {
        &self.cmd
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Response {
    Response(u64, serde_json::Value),
    Notify(String, Vec<serde_json::Value>),
}

pub fn process_connection(
    socket: TcpStream,
    debugger: Arc<Mutex<DebugServer>>,
    notifier: Arc<Mutex<Notifier>>,
) {
    let addr = socket.peer_addr().unwrap();

    let (request_tx, request_rx) = VimCodec::new(notifier.clone(), addr).framed(socket).split();

    let (connection_tx, connection_rx) = mpsc::channel(1);

    notifier
        .lock()
        .unwrap()
        .add_listener(connection_tx.clone(), addr.clone());

    if debugger.lock().unwrap().has_started() {
        notifier.lock().unwrap().signal_started();
    }

    tokio::spawn(
        request_tx
            .send_all(connection_rx.map_err(|e| {
                eprintln!("failed to retrieve message to send: {}", e);
                io::Error::new(io::ErrorKind::Other, e)
            }))
            .then(|res| {
                if let Err(e) = res {
                    match e.kind() {
                        // Remove socket from notifier if pipe broken, otherwise report error
                        std::io::ErrorKind::BrokenPipe => {}
                        _ => {
                            eprintln!("failed to send data to socket; error = {:?}", e);
                        }
                    }
                }

                Ok(())
            }),
    );

    tokio::spawn(
        request_rx
            .and_then(move |req| {
                let debugger = debugger.clone();
                respond(req, debugger, notifier.clone())
            })
            .for_each(move |resp| {
                tokio::spawn(
                    connection_tx
                        .clone()
                        .send(resp)
                        .map(|_| {})
                        .map_err(|e| println!("Error responding: {}", e)),
                );
                Ok(())
            })
            .map_err(|e| {
                eprintln!("Socket error = {:?}", e);
            }),
    );
}

fn respond(
    req: Request,
    debugger: Arc<Mutex<DebugServer>>,
    notifier: Arc<Mutex<Notifier>>,
) -> Box<dyn Future<Item = Response, Error = io::Error> + Send> {
    let json_response = match req.cmd() {
        RequestCmd::Cmd(s) => {
            let s: &str = s;
            match s {
                "ping" => debugger.lock().unwrap().ping(),
                "pings" => debugger.lock().unwrap().pings(),
                _ => return respond_debugger(req, debugger, notifier),
            }
        }
        _ => return respond_debugger(req, debugger, notifier),
    };

    let f = future::lazy(move || match json_response {
        Ok(resp) => Ok(Response::Response(req.id(), resp)),
        Err(_) => {
            unreachable!();
        }
    });

    Box::new(f)
}

fn respond_debugger(
    req: Request,
    debugger: Arc<Mutex<DebugServer>>,
    notifier: Arc<Mutex<Notifier>>,
) -> Box<dyn Future<Item = Response, Error = io::Error> + Send> {
    let id = req.id();

    let f = debugger
        .lock()
        .unwrap()
        .handle(req)
        .timeout(Duration::new(30, 0))
        .then(move |resp| match resp {
            Ok(s) => Ok(Response::Response(id, s)),
            Err(e) => {
                notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::ERROR, format!("{}", e));
                let resp = serde_json::json!({"status":"ERROR"});
                Ok(Response::Response(id, resp))
            }
        });

    return Box::new(f);
}
