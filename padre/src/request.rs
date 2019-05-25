
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

#[derive(Debug)]
pub struct PadreRequest {
}

impl PadreRequest {
    pub fn new(
    ) -> Self {
        PadreRequest {
        }
    }
}

#[cfg(test)]
mod tests {
}
