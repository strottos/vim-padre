//! Handle a connection to the PADRE server including passing messages to and
//! from the debugger to the connection.

use std::env::current_exe;
use std::io;
use std::net::SocketAddr;
use std::process::{exit, Command, Stdio};
use std::str;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::prelude::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::codec::Decoder;

use crate::config::Config;
use crate::debugger::{get_debugger_info, Debugger};
use crate::vimcodec::VimCodec;
use padre_core::debugger::DebuggerCmd;
use padre_core::server::{LogLevel, Notification};
use padre_core::util::{log_msg, serde_json_merge};
use padre_core::Result;

/// Contains command details of a request, either a `PadreCmd` or a `DebuggerCmd`
///
/// Can be of the form of a command without arguments, a command with a location argument or a
/// command with a variable argument.
///
/// Examples:
///
/// ```
/// let command = RequestCmd::PadreCmd(PadreCmd::Ping);
/// let command = RequestCmd::DebuggerCmd(DebuggerCmd::Breakpoint(FileLocation::new("test.c", 12)));
/// let command = RequestCmd::DebuggerCmd(DebuggerCmd::Print(Variable::new("abc")));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum RequestCmd {
    PadreCmd(PadreCmd),
    // 2nd argument is timeout instant value
    DebuggerCmd(DebuggerCmd),
}

/// Contains full details of a request including an id to respond to and a `RequestCmd`
#[derive(Clone, Debug, PartialEq)]
pub struct PadreRequest {
    id: u64,
    cmd: RequestCmd,
}

impl PadreRequest {
    /// Create a request
    pub fn new(id: u64, cmd: RequestCmd) -> Self {
        PadreRequest { id, cmd }
    }

    /// Return the request id
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Return the RequestCmd entry
    pub fn cmd(&self) -> &RequestCmd {
        &self.cmd
    }
}

/// All padre commands
#[derive(Clone, Debug, PartialEq)]
pub enum PadreCmd {
    Ping,
    Pings,
    GetConfig(String),
    SetConfig(String, i64),
}

/// A response to a request
///
/// Takes a u64 as the first argument to represent the id and a JSON object as
/// the second argument to represent the response. For example a response with an id of `1`
/// and a JSON object of `{"status":"OK"}` will be decoded by the `VIMCodec` as
/// `[1,{"status":"OK"}]` and sent as a response to the requesting socket.
///
/// Normally kept simple with important information relegated to an event based notification.
#[derive(Clone, Debug, PartialEq)]
pub struct PadreResponse {
    id: u64,
    resp: serde_json::Value,
}

impl PadreResponse {
    /// Create a response
    pub fn new(id: u64, resp: serde_json::Value) -> Self {
        PadreResponse { id, resp }
    }

    /// Return the response id
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Return the response values
    pub fn resp(&self) -> &serde_json::Value {
        &self.resp
    }
}

/// Data to be sent back to connection in the form of either a `Notification` or a `PadreResponse`
///
/// A `PadreResponse` takes a u64 as the first argument to represent the id and a JSON object as
/// the second argument to represent the response. For example a response with an id of `1`
/// and a JSON object of `{"status":"OK"}` will be decoded by the `VIMCodec` as
/// `[1,{"status":"OK"}]` and sent as a response to the requesting socket.
///
#[derive(Clone, Debug, PartialEq)]
pub enum PadreSend {
    Response(PadreResponse),
    Notification(Notification),
}

pub async fn run(
    connection_addr: SocketAddr,
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
) -> std::result::Result<(), io::Error> {
    let tcp_listener = TcpListener::bind(&connection_addr)
        .map(|tcp_listener| {
            println!("Listening on {}", &connection_addr);
            tcp_listener
        })
        .await
        .expect(&format!("Can't open TCP listener on {}", &connection_addr));

    let (notifier_tx, notifier_rx): (Sender<Notification>, Receiver<Notification>) =
        mpsc::channel(1);
    let (debugger_queue_tx, debugger_queue_rx) = mpsc::channel(1);

    let (debugger_type, debugger_cmd) =
        get_debugger_info(debugger_cmd, debugger_type, &run_cmd).await;

    let (send_stop_tx, send_stop_rx) = oneshot::channel();
    let (recv_stop_tx, recv_stop_rx) = oneshot::channel();

    let mut debugger = Debugger::new(debugger_type, notifier_tx.clone());
    tokio::spawn(async move {
        debugger
            .run(
                debugger_cmd,
                run_cmd,
                debugger_queue_rx,
                send_stop_rx,
                recv_stop_tx,
            )
            .await
    });
    let mut server = Server::new(
        &connection_addr,
        tcp_listener,
        notifier_tx,
        debugger_queue_tx,
    );

    tokio::select! {
        _ = server.run(notifier_rx) => {}
        _ = signal::ctrl_c() => {
            send_stop_tx.send(()).expect("Failed to send shutdown signal to debugger");

            if let Err(_) = timeout(Duration::new(5, 0), recv_stop_rx).await {
                println!("Timed out exiting!");
                exit(-1);
            };

            exit(0);
        }
    }

    Ok(())
}

pub struct Server<'a> {
    addr: &'a SocketAddr,
    tcp_listener: TcpListener,
    listeners: Arc<Mutex<Vec<(Sender<PadreSend>, SocketAddr)>>>,
    notifier_tx: Sender<Notification>,
    debugger_queue_tx: Sender<(
        DebuggerCmd,
        Instant,
        oneshot::Sender<Result<serde_json::Value>>,
    )>,
}

impl<'a> Server<'a> {
    pub fn new(
        addr: &'a SocketAddr,
        tcp_listener: TcpListener,
        notifier_tx: Sender<Notification>,
        debugger_queue_tx: Sender<(
            DebuggerCmd,
            Instant,
            oneshot::Sender<Result<serde_json::Value>>,
        )>,
    ) -> Self {
        Server {
            addr,
            tcp_listener,
            listeners: Arc::new(Mutex::new(vec![])),
            notifier_tx,
            debugger_queue_tx,
        }
    }

    /// Process a TCP listener.
    pub async fn run(&mut self, mut notifier_rx: Receiver<Notification>) {
        let listeners = self.listeners.clone();

        tokio::spawn(async move {
            while let Some(msg) = notifier_rx.recv().await {
                let mut listeners_lock = listeners.lock().unwrap();
                // Expected there's mostly only ever one in here unless debugging or something, but
                // we support more.
                for listener in listeners_lock.iter_mut() {
                    let sender = listener.0.clone();
                    let msg_send = PadreSend::Notification(msg.clone());
                    tokio::spawn(async move {
                        if let Err(_) = sender.send(msg_send).await {
                            // Just skip on, this shouldn't happen very often as we should have
                            // very few listeners
                        }
                    });
                }
            }
        });

        loop {
            let (socket, _) = self.tcp_listener.accept().await.unwrap();
            self.handle(socket).await;
        }
    }

    /// Process a TCP socket connection.
    ///
    /// Fully sets up a new socket connection including listening for requests and sending responses.
    async fn handle(&mut self, stream: TcpStream) {
        let config = Arc::new(Mutex::new(Config::new()));

        let connection_response = ConnectionResponse::new(self.notifier_tx.clone());

        let (mut connection_tx, mut connection_rx) =
            VimCodec::new(config.clone()).framed(stream).split();

        // Just here as an abstraction around the connection so that we can send both responses
        // to requests and to notify of an event (debugger paused for example).
        let (tx, mut rx) = mpsc::channel(1);

        self.add_listener(tx.clone(), self.addr.clone());

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                connection_tx.send(msg).await.unwrap();
            }
        });

        let debugger_queue_tx = self.debugger_queue_tx.clone();

        tokio::spawn(async move {
            while let Some(req) = connection_rx.next().await {
                match req {
                    Ok(req) => {
                        match connection_response.get_response(&req, debugger_queue_tx.clone(), config.clone()).await {
                            Ok(resp) => {
                                tx.clone().send(PadreSend::Response(resp)).await.unwrap()
                            },
                            Err(e) => {
                                tx.clone().send(PadreSend::Response(PadreResponse::new(
                                    req.id(),
                                    serde_json::json!({"status":"ERROR","error":e.get_error_string(),"debug":e.get_debug_string()}),
                                ))).await.unwrap()
                            },
                        };
                    },
                    Err(e) => {
                        tx.clone().send(PadreSend::Response(PadreResponse::new(
                            e.get_id(),
                            serde_json::json!({"status":"ERROR","error":e.get_error_string(),"debug":e.get_debug_string()}),
                        ))).await.unwrap()
                    }
                }
            }
        });

        self.check_for_and_report_padre_updates();
    }

    /// Add a socket as a listener that will be notified of any events that happen in debuggers
    /// asynchronously like code stepping, etc.
    fn add_listener(&mut self, sender: Sender<PadreSend>, addr: SocketAddr) {
        self.listeners.lock().unwrap().push((sender, addr));
    }

    // TODO
    // /// Remove a listener from the notifier
    // ///
    // /// Should be called when a connection is dropped.
    // fn remove_listener(&mut self, addr: &SocketAddr) {
    //     self.listeners
    //         .lock()
    //         .unwrap()
    //         .retain(|listener| listener.1 != *addr);
    // }

    /// Checks whether we're on the latest version with git and if not gives a warning
    fn check_for_and_report_padre_updates(&self) {
        let padre_exe = current_exe().unwrap();
        let padre_dir = padre_exe.parent().unwrap();

        // TODO: Assumes git is used for now and exists, add releasing option in later.
        let output = Command::new("git")
            .arg("status")
            .current_dir(padre_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to execute git command, can't tell if PADRE needs updating");

        let status = str::from_utf8(&output.stdout)
            .unwrap()
            .split('\n')
            .collect::<Vec<&str>>();

        // TODO: Change
        if *status.get(0).unwrap() == "On branch master" {
            Command::new("git")
                .args(vec!["remote", "update"])
                .current_dir(padre_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .expect("Failed to execute git command, can't tell if PADRE needs updating");

            let output = Command::new("git")
                .arg("status")
                .current_dir(padre_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .expect("Failed to execute git command, can't tell if PADRE needs updating");

            let status = str::from_utf8(&output.stdout)
                .unwrap()
                .split('\n')
                .collect::<Vec<&str>>();

            if status.get(1).unwrap().starts_with("Your branch is behind ") {
                println!("Your PADRE version is out of date and should be updated, please run `git pull` in your PADRE directory and and then rerun `make`.");
                log_msg(
                    self.notifier_tx.clone(),
                    LogLevel::WARN,
                    "Your PADRE version is out of date and should be updated, please run `git pull` in your PADRE directory and and then rerun `make`."
                );
            }
        }
    }
}

struct ConnectionResponse {
    notifier_tx: Sender<Notification>,
}

impl ConnectionResponse {
    fn new(notifier_tx: Sender<Notification>) -> Self {
        ConnectionResponse { notifier_tx }
    }

    /// Process a PadreRequest and figure out the response.
    ///
    /// Forwards the request to the appropriate place to handle it and responds appropriately.
    async fn get_response<'a>(
        &self,
        request: &PadreRequest,
        debugger_queue_tx: Sender<(
            DebuggerCmd,
            Instant,
            oneshot::Sender<Result<serde_json::Value>>,
        )>,
        config: Arc<Mutex<Config<'a>>>,
    ) -> Result<PadreResponse> {
        match request.cmd() {
            RequestCmd::PadreCmd(cmd) => {
                let json_response = match cmd {
                    PadreCmd::Ping => self.ping(),
                    PadreCmd::Pings => self.pings(),
                    PadreCmd::GetConfig(key) => self.get_config(config, key),
                    PadreCmd::SetConfig(key, value) => self.set_config(config, key, *value),
                };

                match json_response {
                    Ok(args) => Ok(PadreResponse::new(request.id(), args)),
                    Err(e) => {
                        log_msg(self.notifier_tx.clone(), LogLevel::ERROR, &format!("{}", e));
                        let resp = serde_json::json!({"status":"ERROR"});
                        Ok(PadreResponse::new(request.id(), resp))
                    }
                }
            }
            RequestCmd::DebuggerCmd(cmd) => {
                let (tx, rx) = oneshot::channel();

                let config_timeout: u64 = match cmd {
                    DebuggerCmd::Breakpoint(_) | DebuggerCmd::Unbreakpoint(_) => config
                        .lock()
                        .unwrap()
                        .get_config("BreakpointTimeout")
                        .unwrap()
                        as u64,
                    DebuggerCmd::Run => config
                        .lock()
                        .unwrap()
                        .get_config("ProcessSpawnTimeout")
                        .unwrap() as u64,
                    DebuggerCmd::Print(_) => config
                        .lock()
                        .unwrap()
                        .get_config("PrintVariableTimeout")
                        .unwrap() as u64,
                    DebuggerCmd::StepIn(_)
                    | DebuggerCmd::StepOver(_)
                    | DebuggerCmd::StepOut
                    | DebuggerCmd::Continue => {
                        config.lock().unwrap().get_config("StepTimeout").unwrap() as u64
                    }
                };
                let timeout = Instant::now() + Duration::new(config_timeout, 0);

                match debugger_queue_tx.send((cmd.clone(), timeout, tx)).await {
                    Ok(_) => {}
                    Err(e) => {
                        return Ok(PadreResponse::new(
                            request.id(),
                            serde_json::json!({
                                "status": "ERROR",
                                "error": "Error sending message to debugger",
                                "debug": format!("Error sending message to debugger: {}", e)
                            }),
                        ))
                    }
                };

                match rx.await {
                    Ok(msg) => match msg {
                        Ok(msg) => {
                            let mut ret = serde_json::json!({"status":"OK"});
                            serde_json_merge(&mut ret, msg);
                            Ok(PadreResponse::new(request.id(), ret))
                        }
                        Err(e) => Ok(PadreResponse::new(
                            request.id(),
                            serde_json::json!({"status":"ERROR","error":e.get_error_string(),"debug":e.get_debug_string()}),
                        )),
                    },
                    Err(e) => Ok(PadreResponse::new(
                        request.id(),
                        serde_json::json!({"status":"ERROR","error":"Didn't recieve done notification","debug":&format!("{:?}", e)}),
                    )),
                }
            }
        }
    }

    fn ping(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({"status":"OK","ping":"pong"}))
    }

    fn pings(&self) -> Result<serde_json::Value> {
        log_msg(self.notifier_tx.clone(), LogLevel::INFO, "pong");

        Ok(serde_json::json!({"status":"OK"}))
    }

    fn get_config(&self, config: Arc<Mutex<Config>>, key: &str) -> Result<serde_json::Value> {
        let value = config.lock().unwrap().get_config(key);
        match value {
            Some(v) => Ok(serde_json::json!({"status":"OK","value":v})),
            None => Ok(serde_json::json!({"status":"ERROR"})),
        }
    }

    fn set_config(
        &self,
        config: Arc<Mutex<Config>>,
        key: &str,
        value: i64,
    ) -> Result<serde_json::Value> {
        let config_set = config.lock().unwrap().set_config(key, value);
        match config_set {
            true => Ok(serde_json::json!({"status":"OK"})),
            false => Ok(serde_json::json!({"status":"ERROR"})),
        }
    }
}
