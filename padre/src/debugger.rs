//! debugger

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::request::{RequestError, Response};
use crate::notifier::{LogLevel, Notifier};

mod lldb;

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    fn set_path() {
        let test_files_path_raw = String::from("./test_files/");
        let test_files_path = Path::new(&test_files_path_raw)
                                   .canonicalize()
                                   .expect("Cannot find test_files directory");
        let path_var = format!("/bin:{}:/usr/bin", test_files_path.as_path().to_str().unwrap());
        env::set_var("PATH", &path_var);
    }

    #[test]
    fn finds_lldb_when_specified_and_in_path() {
        set_path();
        let v = vec!["lldb-server".to_string(), "test".to_string()];
        assert_eq!(super::get_debugger_type(&v), Some(String::from("lldb")));
    }

    #[test]
    fn finds_lldb_when_specified_and_absolute_path() {
        let v = vec!["./test_files/lldb-server".to_string(), "test".to_string()];
        assert_eq!(super::get_debugger_type(&v), Some(String::from("lldb")));
    }

    #[test]
    fn finds_lldb_when_elf_file() {
        let v = vec!["./test_files/hello_world".to_string()];
        assert_eq!(super::get_debugger_type(&v), Some(String::from("lldb")));
    }

    #[test]
    #[should_panic]
    fn errors_when_program_not_found() {
        let v = vec!["program-not-exists".to_string()];
        assert_eq!(super::get_debugger_type(&v), Some(String::from("EXPECT PANIC")));
    }

//    #[test]
//    fn finds_node_when_node_program() {
//        set_path();
//        let v = vec!["node", "./test_files/test_node.js"];
//        assert_eq!(super::get_debugger_type(v), Some(String::from("node")));
//    }
//
//    #[test]
//    fn finds_node_when_js_file() {
//        let v = vec!["./test_files/test_node.js"];
//        assert_eq!(super::get_debugger_type(v), Some(String::from("node")));
//    }
}

pub trait Debugger {
    fn start(&mut self, debugger_command: String, run_command: &Vec<String>);
    fn has_started(&self) -> bool;
    fn stop(&self);
    fn breakpoint(&self, file: String, line_num: u32) -> Result<Response<Option<String>>, RequestError>;
}

pub struct PadreServer {
    notifier: Arc<Mutex<Notifier>>,
    pub debugger: Arc<Mutex<dyn Debugger + Send>>,
}

impl PadreServer {
    pub fn new(notifier: Arc<Mutex<Notifier>>, debugger: Arc<Mutex<dyn Debugger + Send>>) -> PadreServer {
        PadreServer {
            notifier: notifier,
            debugger: debugger,
        }
    }

    pub fn start(&self, debugger_command: String, run_command: &Vec<String>) {
        self.debugger.lock().unwrap().start(debugger_command, run_command);
    }

    pub fn ping(&self) -> Result<Response<Option<String>>, RequestError> {
        Ok(Response::OK(Some(String::from("pong"))))
    }

    pub fn pings(&self) -> Result<Response<Option<String>>, RequestError> {
        // TODO: Better than unwrap?
        self.notifier.lock().unwrap().log_msg(LogLevel::INFO, "pong".to_string());
        Ok(Response::OK(None))
    }
}

pub fn get_debugger(cmd: &Vec<String>, debugger_type: Option<&str>, notifier: Arc<Mutex<Notifier>>) -> PadreServer {
    let debugger_type = match debugger_type {
        Some(s) => s.to_string(),
        None => get_debugger_type(cmd).expect("Can't find debugger type, bailing"),
    };

    let debug_server = match debugger_type.to_ascii_lowercase().as_ref() {
        "lldb" => {
            Arc::new(Mutex::new(lldb::LLDB::new(Arc::clone(&notifier))))
        }
        _ => panic!("Can't build debugger, panicking"),
    };

    PadreServer::new(notifier, debug_server)
}

pub fn get_debugger_type(cmd: &Vec<String>) -> Option<String> {
//    if is_node(cmd[0]) {
//        Some(String::from("node"))
//    } else
    if is_lldb(&cmd[0]) {
        Some(String::from("lldb"))
    } else {
        None
    }
}

fn find_cmd_full_path(cmd: &str) -> String {
    let cmd_full_path_buf = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).filter_map(|dir| {
            let cmd_full_path = dir.join(&cmd);
            if cmd_full_path.is_file() {
                Some(cmd_full_path)
            } else {
                None
            }
        }).next()
    }).unwrap_or(PathBuf::from(cmd));
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

    if cmd_file_type.contains("ELF") {
        return true
    }

    false
}
//
//fn is_node(cmd: &str) -> bool {
//    let cmd_file_type = find_file_type(cmd);
//
//    if (cmd_file_type.contains("ASCII") || cmd_file_type.contains("UTF-8")) && cmd.ends_with(".js") {
//        return true
//    }
//
//    if cmd_file_type.contains("ELF") && cmd == "node" {
//        return true
//    }
//
//    false
//}
