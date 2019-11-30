//! Server
//!
//! Handles the main network connections, parses basic messages and forwards to
//! padre and debuggers for actioning.

use std::env::current_exe;
use std::io;
use std::process::{Command, Stdio};
use std::str;

use crate::config::Config;
// TODO: Add in remove_listener
use crate::notifier::{add_listener, log_msg, LogLevel};
use crate::vimcodec::VimCodec;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::Decoder;

// TODO: Get some of this out of pub use and just in this module?

/// All debugger commands
#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum DebuggerCmd {
    Run,
    Breakpoint(FileLocation),
    StepIn,
    StepOver,
    Continue,
    Print(Variable),
}

/// File location
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct FileLocation {
    name: String,
    line_num: u64,
}

impl FileLocation {
    pub fn new(name: String, line_num: u64) -> Self {
        FileLocation { name, line_num }
    }
}

/// Variable name
#[derive(Clone, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub fn new(name: String) -> Self {
        Variable { name }
    }
}

/// All padre commands
#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum PadreCmd {
    Ping,
    Pings,
    GetConfig(String),
    SetConfig(String, i64),
}

/// Contains command details of a request, either a `PadreCmd` or a `DebuggerCmd`
///
/// Can be of the form of a command without arguments, a command with a location argument or a
/// command with a variable argument.
///
/// Examples:
///
/// ```
/// use padre_core::server::{RequestCmd, DebuggerCmd, FileLocation, Variable};
///
/// let command = RequestCmd::DebuggerCmd(DebuggerCmd::Run);
///
/// let file_location = FileLocation::new("test.c".to_string(), 12);
/// let command = RequestCmd::DebuggerCmd(DebuggerCmd::Breakpoint(file_location));
///
/// let variable = Variable::new("abc".to_string());
/// let command = RequestCmd::DebuggerCmd(DebuggerCmd::Print(variable));
/// ```
#[derive(Clone, Deserialize, Debug, PartialEq)]
pub enum RequestCmd {
    PadreCmd(PadreCmd),
    DebuggerCmd(DebuggerCmd),
}

/// Contains full details of a request including an id to respond to and a `RequestCmd`
#[derive(Deserialize, Debug, PartialEq)]
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

/// A response to a request
///
/// Takes a u64 as the first argument to represent the id and a JSON object as
/// the second argument to represent the response. For example a response with an id of `1`
/// and a JSON object of `{"status":"OK"}` will be decoded by the `VIMCodec` as
/// `[1,{"status":"OK"}]` and sent as a response to the requesting socket.
///
/// Normally kept simple with important information relegated to an event based notification.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Response {
    id: u64,
    resp: serde_json::Value,
}

impl Response {
    /// Create a response
    pub fn new(id: u64, resp: serde_json::Value) -> Self {
        Response { id, resp }
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

/// A notification to be sent to all listeners of an event
///
/// Takes a String as the command and a vector of JSON values as arguments. For example, a
/// `Notication` with a command `execute` and vector arguments TODO...
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Notification {
    cmd: String,
    args: Vec<serde_json::Value>,
}

impl Notification {
    /// Create a notification
    pub fn new(cmd: String, args: Vec<serde_json::Value>) -> Self {
        Notification { cmd, args }
    }

    /// Return the notification cmd
    pub fn cmd(&self) -> &str {
        self.cmd.as_ref()
    }

    /// Return the response values
    pub fn args(&self) -> &Vec<serde_json::Value> {
        &self.args
    }
}

/// Data to be sent back to connection in the form of either a `Notification` or a `Response`
///
/// A `Response` takes a u64 as the first argument to represent the id and a JSON object as
/// the second argument to represent the response. For example a response with an id of `1`
/// and a JSON object of `{"status":"OK"}` will be decoded by the `VIMCodec` as
/// `[1,{"status":"OK"}]` and sent as a response to the requesting socket.
///
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum PadreSend {
    Response(Response),
    Notification(Notification),
}

/// Process a TCP socket connection.
///
/// Fully sets up a new socket connection including listening for requests and sending responses.
pub fn process_connection(stream: TcpStream, debugger_queue_tx: Sender<DebuggerCmd>) {
    let addr = stream.peer_addr().unwrap();

    let mut config = Config::new();

    let (mut request_tx, mut request_rx) = VimCodec::new().framed(stream).split();

    let (connection_tx, mut connection_rx) = mpsc::channel(1);

    add_listener(connection_tx.clone(), addr.clone());

    tokio::spawn(async move {
        while let Some(msg) = connection_rx.next().await {
            request_tx.send(msg).await.unwrap();
        }
    });

    tokio::spawn(async move {
        while let Some(req) = request_rx.next().await {
            let resp = respond(req.unwrap(), debugger_queue_tx.clone(), &mut config).await.unwrap();
            connection_tx
                .clone()
                .send(PadreSend::Response(resp))
                .await
                .unwrap();
        }
    });

    tokio::spawn(check_for_and_report_padre_updates());
}

/// Process a PadreRequest.
///
/// Forwards the request to the appropriate place to handle it and responds appropriately.
async fn respond<'a>(
    request: PadreRequest,
    mut debugger_queue_tx: Sender<DebuggerCmd>,
    config: &mut Config<'a>,
) -> Result<Response, io::Error> {
    match request.cmd() {
        RequestCmd::PadreCmd(cmd) => {
            let json_response = match cmd {
                PadreCmd::Ping => ping(),
                PadreCmd::Pings => pings(),
                PadreCmd::GetConfig(key) => get_config(config, key),
                PadreCmd::SetConfig(key, value) => set_config(config, key, *value),
            };

            match json_response {
                Ok(args) => Ok(Response::new(request.id(), args)),
                Err(e) => {
                    log_msg(LogLevel::ERROR, &format!("{}", e));
                    let resp = serde_json::json!({"status":"ERROR"});
                    Ok(Response::new(request.id(), resp))
                }
            }
        }
        RequestCmd::DebuggerCmd(cmd) => {
            debugger_queue_tx.send(cmd.clone()).await.unwrap();
            Ok(Response::new(request.id(), serde_json::json!({"status":"OK"})))
        }
    }
}

fn ping() -> Result<serde_json::Value, io::Error> {
    Ok(serde_json::json!({"status":"OK","ping":"pong"}))
}

fn pings() -> Result<serde_json::Value, io::Error> {
    log_msg(LogLevel::INFO, "pong");

    Ok(serde_json::json!({"status":"OK"}))
}

fn get_config(config: &Config, key: &str) -> Result<serde_json::Value, io::Error> {
    let value = config.get_config(key);
    match value {
        Some(v) => Ok(serde_json::json!({"status":"OK","value":v})),
        None => Ok(serde_json::json!({"status":"ERROR"})),
    }
}

fn set_config(
    config: &mut Config,
    key: &str,
    value: i64,
) -> Result<serde_json::Value, io::Error> {
    let config_set = config.set_config(key, value);
    match config_set {
        true => Ok(serde_json::json!({"status":"OK"})),
        false => Ok(serde_json::json!({"status":"ERROR"})),
    }
}

/// Checks whether we're on the latest version with git and if not gives a warning
async fn check_for_and_report_padre_updates() {
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
            log_msg(LogLevel::WARN, "Your PADRE version is out of date and should be updated, please run `git pull` in your PADRE directory and and then rerun `make`.");
        }
    }
}
