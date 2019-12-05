//! Node message analyser
//!
//! Analyses the messages that come from the WebSocket connection to Node Debugger

use std::sync::{Arc, Mutex};

use super::ws::WSHandler;
use padre_core::notifier::{jump_to_position, log_msg, signal_exited, LogLevel};
use padre_core::server::FileLocation;

use tokio::prelude::*;

/// Node script, indicated by receiving a 'Debugger.scriptParsed' message from Node
#[derive(Debug, Eq, PartialEq)]
pub struct Script {
    file: String,
    script_id: String,
    is_internal: bool,
}

impl Script {
    pub fn new(file: String, script_id: String, is_internal: bool) -> Self {
        Script {
            file,
            script_id,
            is_internal,
        }
    }

    pub fn get_script_id(&self) -> &str {
        &self.script_id
    }
}

#[derive(Debug)]
pub struct Analyser {
    scripts: Vec<Script>,
    pending_breakpoints: Vec<FileLocation>,
    ws_handler: Arc<Mutex<WSHandler>>,
    pid: Option<u64>,
}

impl Analyser {
    pub fn new(ws_handler: Arc<Mutex<WSHandler>>) -> Self {
        Analyser {
            scripts: vec![],
            pending_breakpoints: vec![],
            ws_handler,
            pid: None,
        }
    }

    pub fn analyse_message(&mut self, mut msg: serde_json::Value) {
        let method: String = match serde_json::from_value(msg["method"].take()) {
            Ok(s) => s,
            Err(e) => {
                panic!("Can't understand method: {:?}", e);
            }
        };

        match method.as_ref() {
            "Runtime.consoleAPICalled" => {}
            "Runtime.executionContextCreated" => {}
            "Runtime.executionContextDestroyed" => {
                match self.pid {
                    Some(pid) => signal_exited(pid, 0),
                    None => {}
                };
                self.ws_handler.lock().unwrap().close()
            }
            "Runtime.exceptionThrown" => println!("TODO: Code {:?}", msg),
            "Debugger.paused" => self.analyse_debugger_paused(msg),
            "Debugger.resumed" => {}
            "Debugger.scriptFailedToParse" => {
                log_msg(LogLevel::WARN, &format!("Can't parse script: {:?}", msg))
            }
            "Debugger.scriptParsed" => self.analyse_script_parsed(msg),
            _ => panic!("Can't understand message type: {:?}", method),
        }
    }

    pub fn get_script_from_filename(&self, filename: &str) -> Option<&Script> {
        for script in &self.scripts {
            if &script.file == filename {
                return Some(script);
            }
        }
        None
    }

    pub fn add_pending_breakpoint(&mut self, bkpt: FileLocation) {
        self.pending_breakpoints.push(bkpt);
    }

    pub fn set_pid(&mut self, pid: u64) {
        self.pid = Some(pid);
    }

    fn analyse_script_parsed(&mut self, mut msg: serde_json::Value) {
        let mut is_internal = true;

        let file: String = match serde_json::from_value(msg["params"]["url"].take()) {
            Ok(s) => {
                let mut s: String = s;
                if s.len() > 7 && &s[0..7] == "file://" {
                    is_internal = false;
                    s.replace_range(0..7, "");
                }
                s
            }
            Err(e) => {
                panic!("Can't understand file: {:?}", e);
            }
        };

        let script_id: String = match serde_json::from_value(msg["params"]["scriptId"].take()) {
            Ok(s) => s,
            Err(e) => {
                panic!("Can't understand script_id: {:?}", e);
            }
        };

        // TODO: drain_filter if/when it's stable in Rust
        let mut i = 0;

        while i != self.pending_breakpoints.len() {
            //if self.pending_breakpoints[i].name() == file {
            //    let bkpt = self.pending_breakpoints.remove(i);

            //    let msg = OwnedMessage::Text(format!(
            //        "{{\
            //         \"method\":\"Debugger.setBreakpoint\",\
            //         \"params\":{{\
            //         \"location\":{{\
            //         \"scriptId\":\"{}\",\
            //         \"lineNumber\":{}\
            //         }}\
            //         }}\
            //         }}",
            //        script_id,
            //        bkpt.line_num() - 1
            //    ));

            //    let file = file.clone();

            //    let ws_handler = self.ws_handler.clone();

            //    tokio::spawn(
            //        ws_handler
            //            .lock()
            //            .unwrap()
            //            .send_and_receive_message(msg)
            //            .map(move |response| {
            //                if response["error"].is_null() {
            //                    //breakpoint_set(&file, bkpt.line_num);
            //                } else {
            //                    log_msg(
            //                        LogLevel::CRITICAL,
            //                        &format!("Can't set breakpoint {:?}", bkpt),
            //                    );
            //                    panic!("Can't set breakpoint, panicking");
            //                }
            //            })
            //            .map_err(|e| {
            //                log_msg(
            //                    LogLevel::CRITICAL,
            //                    &format!("Can't set breakpoint, error: {}", e),
            //                );
            //                panic!("Can't set breakpoint, panicking");
            //            }),
            //    );
            //} else {
            //    i += 1;
            //}
        }

        self.scripts.push(Script::new(file, script_id, is_internal));
    }

    fn analyse_debugger_paused(&self, mut msg: serde_json::Value) {
        let file: String =
            match serde_json::from_value(msg["params"]["callFrames"][0]["url"].take()) {
                Ok(s) => {
                    let mut s: String = s;
                    if s.len() > 7 && &s[0..7] == "file://" {
                        s = s[7..].to_string()
                    }
                    s
                }
                Err(e) => {
                    // TODO: How do we get here? Handle when we see it.
                    panic!("JSON: {}, err: {}", msg, e);
                }
            };

        let line_num: u64 = match serde_json::from_value(
            msg["params"]["callFrames"][0]["location"]["lineNumber"].take(),
        ) {
            Ok(s) => {
                let s: u64 = s;
                s + 1
            }
            Err(e) => {
                panic!("Can't understand line_num: {:?}", e);
            }
        };

        jump_to_position(&file, line_num);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::super::ws::WSHandler;
    use super::Analyser;

    #[test]
    fn check_internal_script_parsed() {
        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params": {
                "scriptId":"7",
                "url":"internal/bootstrap/loaders.js",
                "startLine":0,
                "startColumn":0,
                "endLine":312,
                "endColumn":0,
                "executionContextId":1,
                "hash":"39ff95c38ab7c4bb459aabfe5c5eb3a27441a4d8",
                "executionContextAuxData":{
                  "isDefault":true
                },
                "isLiveEdit":false,
                "sourceMapURL":"",
                "hasSourceURL":false,
                "isModule":false,
                "length":9613
              }
            }
        );
        let ws = Arc::new(Mutex::new(WSHandler::new()));
        let mut analyser = Analyser::new(ws);

        analyser.analyse_message(msg);

        assert_eq!(analyser.scripts.len(), 1);
        assert_eq!(
            analyser.scripts[0].file,
            "internal/bootstrap/loaders.js".to_string()
        );
        assert_eq!(analyser.scripts[0].script_id, "7".to_string());
        assert_eq!(analyser.scripts[0].is_internal, true);

        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params":{
                "scriptId":"8",
                "url":"internal/bootstrap/node.js",
                "startLine":0,
                "startColumn":0,
                "endLine":438,
                "endColumn":0,
                "executionContextId":1,
                "hash":"3f184a9d8a71f2554b8b31895d935027129c91c4",
                "executionContextAuxData":{
                  "isDefault":true
                },
                "isLiveEdit":false,
                "sourceMapURL":"",
                "hasSourceURL":false,
                "isModule":false,
                "length":14904
              }
            }
        );

        analyser.analyse_message(msg);

        assert_eq!(analyser.scripts.len(), 2);
        assert_eq!(
            analyser.scripts[1].file,
            "internal/bootstrap/node.js".to_string()
        );
        assert_eq!(analyser.scripts[1].script_id, "8".to_string());
        assert_eq!(analyser.scripts[1].is_internal, true);
    }

    #[test]
    fn check_file_script_parsed() {
        let msg = serde_json::json!(
            {
              "method":"Debugger.scriptParsed",
              "params":{
                "scriptId":"52",
                "url":"file:///home/me/test.js"
              }
            }
        );

        let ws = Arc::new(Mutex::new(WSHandler::new()));
        let mut analyser = Analyser::new(ws);

        analyser.analyse_message(msg);

        assert_eq!(analyser.scripts.len(), 1);
        assert_eq!(analyser.scripts[0].file, "/home/me/test.js".to_string());
        assert_eq!(analyser.scripts[0].script_id, "52".to_string());
        assert_eq!(analyser.scripts[0].is_internal, false);
    }

    #[test]
    fn test_get_existing_script_from_filename() {
        let ws = Arc::new(Mutex::new(WSHandler::new()));
        let mut analyser = Analyser::new(ws);
        let script = super::Script::new("exists.js".to_string(), "52".to_string(), false);
        let expected_script = super::Script::new("exists.js".to_string(), "52".to_string(), false);
        analyser.scripts.push(script);
        assert_eq!(
            analyser.get_script_from_filename("exists.js").unwrap(),
            &expected_script
        );
    }

    #[test]
    fn test_get_no_script_from_filename() {
        let ws = Arc::new(Mutex::new(WSHandler::new()));
        let analyser = Analyser::new(ws);
        assert_eq!(analyser.get_script_from_filename("not_exists.js"), None);
    }
}
