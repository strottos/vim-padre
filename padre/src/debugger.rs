mod lldb;
mod node;
mod tty_process;

use std::env;
use std::fmt::Debug;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};
use crate::server::{Request, RequestCmd};

use tokio::prelude::*;

pub fn get_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
    notifier: Arc<Mutex<Notifier>>,
) -> DebugServer {
    let debugger_type = match debugger_type {
        Some(s) => s.to_string(),
        None => match debugger_cmd {
            Some(s) => get_debugger_type(s).expect("Can't find debugger type, bailing"),
            None => panic!("Couldn't find debugger, try specifying with -t or -d"),
        },
    };

    let debugger_cmd = match debugger_cmd {
        Some(s) => s.to_string(),
        None => debugger_type.clone(),
    };

    let mut debugger: Box<dyn Debugger + Send> = match debugger_type.to_ascii_lowercase().as_ref() {
        "lldb" => Box::new(lldb::ImplDebugger::new(
            notifier.clone(),
            debugger_cmd,
            run_cmd,
        )),
        "node" => Box::new(node::ImplDebugger::new(
            notifier.clone(),
            debugger_cmd,
            run_cmd,
        )),
        _ => panic!("Can't build debugger type {}, panicking", &debugger_type),
    };

    debugger.setup();

    DebugServer::new(notifier, debugger)
}

pub fn get_debugger_type(cmd: &str) -> Option<String> {
    if is_node(&cmd) {
        Some(String::from("node"))
    } else if is_lldb(&cmd) {
        Some(String::from("lldb"))
    } else {
        None
    }
}

fn find_cmd_full_path(cmd: &str) -> String {
    let cmd_full_path_buf = env::var_os("PATH")
        .and_then(|paths| {
            env::split_paths(&paths)
                .filter_map(|dir| {
                    let cmd_full_path = dir.join(&cmd);
                    if cmd_full_path.is_file() {
                        Some(cmd_full_path)
                    } else {
                        None
                    }
                })
                .next()
        })
        .unwrap_or(PathBuf::from(cmd));
    String::from(cmd_full_path_buf.as_path().to_str().unwrap())
}

fn find_file_type(cmd: &str) -> String {
    let cmd_full_path = find_cmd_full_path(cmd);

    let output = Command::new("file")
        .arg("-L") // Follow symlinks
        .arg(&cmd_full_path)
        .output()
        .ok()
        .expect(&format!("Can't run file on {} to find file type", cmd));

    String::from_utf8_lossy(&output.stdout).to_string()
}

fn is_lldb(cmd: &str) -> bool {
    let cmd_file_type = find_file_type(cmd);

    if cmd_file_type.contains("ELF") || cmd.contains("lldb") {
        return true;
    }

    false
}

fn is_node(cmd: &str) -> bool {
    let cmd_file_type = find_file_type(cmd);

    if (cmd_file_type.contains("ASCII") || cmd_file_type.contains("UTF-8")) && cmd.ends_with(".js")
    {
        return true;
    }

    if cmd_file_type.contains("ELF") && cmd == "node" {
        return true;
    }

    false
}

pub trait Debugger: Debug {
    fn setup(&mut self);
    fn teardown(&mut self);
    fn has_started(&self) -> bool;
    fn run(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn breakpoint(
        &mut self,
        file: String,
        line_num: u64,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_in(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn step_over(&mut self) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn continue_on(
        &mut self,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
    fn print(
        &mut self,
        variable: &str,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send>;
}

#[derive(Debug)]
pub struct DebugServer {
    notifier: Arc<Mutex<Notifier>>,
    debugger: Box<dyn Debugger + Send>,
}

impl DebugServer {
    pub fn new(notifier: Arc<Mutex<Notifier>>, debugger: Box<dyn Debugger + Send>) -> DebugServer {
        DebugServer { notifier, debugger }
    }

    pub fn ping(&self) -> Result<serde_json::Value, io::Error> {
        let pong = serde_json::json!({"status":"OK","ping":"pong"});
        Ok(pong)
    }

    pub fn pings(&self) -> Result<serde_json::Value, io::Error> {
        let pongs = serde_json::json!({"status":"OK"});
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "pong".to_string());
        Ok(pongs)
    }

    pub fn has_started(&self) -> bool {
        self.debugger.has_started()
    }

    pub fn handle(
        &mut self,
        req: Request,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        match req.cmd() {
            RequestCmd::Cmd(cmd) => {
                let cmd: &str = cmd;
                match cmd {
                    "run" => self.debugger.run(),
                    "stepIn" => self.debugger.step_in(),
                    "stepOver" => self.debugger.step_over(),
                    "continue" => self.debugger.continue_on(),
                    _ => self.send_error_and_debug(
                        format!("Can't understand request"),
                        format!("Can't understand command without arguments: '{}'", cmd),
                    ),
                }
            }
            RequestCmd::CmdWithFileLocation(cmd, file, line) => {
                let cmd: &str = cmd;
                match cmd {
                    "breakpoint" => self.debugger.breakpoint(file.clone(), *line),
                    _ => self.send_error_and_debug(
                        format!("Can't understand command"),
                        format!("Can't understand command '{}' with file location", cmd),
                    ),
                }
            }
            RequestCmd::CmdWithVariable(cmd, variable) => {
                let cmd: &str = cmd;
                match cmd {
                    "print" => self.debugger.print(variable),
                    _ => self.send_error_and_debug(
                        format!("Can't understand command"),
                        format!("Can't understand command '{}' with variable", cmd),
                    ),
                }
            }
        }
    }

    pub fn stop(&mut self) {
        self.debugger.teardown();
    }

    fn send_error_and_debug(
        &self,
        err_msg: String,
        debug_msg: String,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::ERROR, err_msg);
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::DEBUG, debug_msg);

        let f = future::lazy(move || {
            let resp = serde_json::json!({"status":"ERROR"});
            Ok(resp)
        });

        Box::new(f)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    fn set_path() {
        let test_files_path_raw = String::from("./test_files/");
        let test_files_path = Path::new(&test_files_path_raw)
            .canonicalize()
            .expect("Cannot find test_files directory");
        let path_var = format!(
            "/bin:{}:/usr/bin",
            test_files_path.as_path().to_str().unwrap()
        );
        env::set_var("PATH", &path_var);
    }

    #[test]
    fn finds_lldb_when_specified_and_in_path() {
        set_path();
        assert_eq!(
            super::get_debugger_type("lldb-server"),
            Some(String::from("lldb"))
        );
    }

    #[test]
    fn finds_lldb_when_specified_and_absolute_path() {
        assert_eq!(
            super::get_debugger_type("./test_files/lldb-server"),
            Some(String::from("lldb"))
        );
    }

    #[test]
    fn finds_lldb_when_elf_file() {
        assert_eq!(
            super::get_debugger_type("./test_files/hello_world"),
            Some(String::from("lldb"))
        );
    }

    #[test]
    #[should_panic]
    fn errors_when_program_not_found() {
        assert_eq!(
            super::get_debugger_type("program-not-exists"),
            Some(String::from("EXPECT PANIC"))
        );
    }

    #[test]
    fn finds_node_when_node_program() {
        set_path();
        assert_eq!(super::get_debugger_type("node"), Some(String::from("node")));
    }

    #[test]
    fn finds_node_when_js_file() {
        assert_eq!(
            super::get_debugger_type("./test_files/test_node.js"),
            Some(String::from("node"))
        );
    }
}
