use std::collections::HashMap;
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PadreRequest {
    id: u32,
    cmd: String,
    //    args: HashMap<String, String>,
}

impl PadreRequest {
    pub fn new(
        id: u32,
        cmd: String,
        //        args: HashMap<String, String>,
    ) -> Self {
        PadreRequest {
            id,
            cmd,
            //            args,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn cmd(&self) -> &str {
        &self.cmd
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PadreResponse {
    id: u32,
    json: serde_json::Value,
}

impl PadreResponse {
    pub fn new(id: u32, json: serde_json::Value) -> Self {
        PadreResponse {
            id,
            json,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn json(&self) -> &serde_json::Value {
        &self.json
    }
}

#[cfg(test)]
mod tests {}
