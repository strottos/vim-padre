//! handle server connections

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::debugger::PadreDebugger;
use crate::notifier::{LogLevel, Notifier};
use crate::vimcodec::VimCodec;

use tokio::codec::Decoder;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::sync::mpsc;

#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum PadreRequestCmd {
    Cmd(String),
    CmdWithFileLocation(String, String, u64),
    CmdWithVariable(String, String),
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct PadreRequest {
    id: u64,
    cmd: PadreRequestCmd,
}

impl PadreRequest {
    pub fn new(id: u64, cmd: PadreRequestCmd) -> Self {
        PadreRequest { id, cmd }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn cmd(&self) -> &PadreRequestCmd {
        &self.cmd
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum PadreResponse {
    Response(u64, serde_json::Value),
    Notify(String, Vec<serde_json::Value>),
}

pub fn process_connection(
    socket: TcpStream,
    debugger: Arc<Mutex<PadreDebugger>>,
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
                eprintln!("failed to accept socket; error = {:?}", e);
            }),
    );
}

fn respond(
    req: PadreRequest,
    debugger: Arc<Mutex<PadreDebugger>>,
    notifier: Arc<Mutex<Notifier>>,
) -> Box<dyn Future<Item = PadreResponse, Error = io::Error> + Send> {
    let json_response = match req.cmd() {
        PadreRequestCmd::Cmd(s) => {
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
        Ok(resp) => Ok(PadreResponse::Response(req.id(), resp)),
        Err(_) => {
            unreachable!();
        }
    });

    Box::new(f)
}

fn respond_debugger(
    req: PadreRequest,
    debugger: Arc<Mutex<PadreDebugger>>,
    notifier: Arc<Mutex<Notifier>>,
) -> Box<dyn Future<Item = PadreResponse, Error = io::Error> + Send> {
    let id = req.id();

    let f = debugger
        .lock()
        .unwrap()
        .handle(req)
        .timeout(Duration::new(30, 0))
        .then(move |resp| match resp {
            Ok(s) => Ok(PadreResponse::Response(id, s)),
            Err(e) => {
                notifier
                    .lock()
                    .unwrap()
                    .log_msg(LogLevel::ERROR, format!("{}", e));
                let resp = serde_json::json!({"status":"ERROR"});
                Ok(PadreResponse::Response(id, resp))
            }
        });

    return Box::new(f);
}
