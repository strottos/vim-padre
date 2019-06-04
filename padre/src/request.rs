use std::error::Error;
use std::fmt;

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
