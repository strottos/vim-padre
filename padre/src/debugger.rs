//! debugger

mod lldb;

use std::env;
use std::fmt::Debug;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};
use crate::request::{RequestError, Response};

use bytes::Bytes;
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug)]
pub enum DebuggerInstruction {
    Run,
    Breakpoint,
}

#[derive(Debug)]
pub enum DebuggerState {
    Stopped,
    Paused(String, u32),
    Running,
    Error,
}

//pub trait DebuggerTrait: Debug {
//    fn run(&mut self) -> Result<Response<json::object::Object>, RequestError>;
//    fn breakpoint(
//        &mut self,
//        file: String,
//        line_num: u32,
//    ) -> Result<Response<json::object::Object>, RequestError>;
//    fn step_in(&mut self) -> Result<Response<json::object::Object>, RequestError>;
//    fn step_over(&mut self) -> Result<Response<json::object::Object>, RequestError>;
//    fn continue_on(&mut self) -> Result<Response<json::object::Object>, RequestError>;
//    fn print(&mut self, variable: String) -> Result<Response<json::object::Object>, RequestError>;
//}

pub trait ProcessTrait: Debug {
    fn start(&mut self);
    fn has_started(&self) -> bool;
    // TODO: fn listener(&self, Sender<Bytes>, Condvar
    fn stop(&self);
}

#[derive(Debug)]
pub struct PadreDebugger {
    notifier: Arc<Mutex<Notifier>>,
    debugger_tx: Sender<DebuggerInstruction>,
}

impl PadreDebugger {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_tx: Sender<DebuggerInstruction>,
    ) -> PadreDebugger {
        PadreDebugger { notifier, debugger_tx }
    }

    pub fn ping(&self) -> Result<Response<json::object::Object>, RequestError> {
        let mut pong = json::object::Object::new();
        pong.insert("ping", json::from("pong".to_string()));
        Ok(Response::OK(pong))
    }

    pub fn pings(&self) -> Result<Response<json::object::Object>, RequestError> {
        // TODO: Better than unwrap?
        let pongs = json::object::Object::new();
        self.notifier
            .lock()
            .unwrap()
            .log_msg(LogLevel::INFO, "pong".to_string());
        Ok(Response::OK(pongs))
    }

    pub fn run(&mut self) -> Result<Response<json::object::Object>, RequestError> {
        println!("RUNNING");
        let mut ret = json::object::Object::new();

        self.debugger_tx.try_send(DebuggerInstruction::Run).unwrap();

        ret.insert("pid", json::from("0".to_string()));

        Ok(Response::OK(ret))
    }
}

impl Future for PadreDebugger {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        println!("TESTING");
        Ok(Async::Ready(()))
    }
}

#[derive(Debug)]
pub struct PadreProcess {
    process: Arc<Mutex<dyn ProcessTrait + Send>>,
}

impl PadreProcess {
    pub fn new(process: Arc<Mutex<dyn ProcessTrait + Send>>) -> PadreProcess {
        PadreProcess { process }
    }
}

impl Future for PadreProcess {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.process.lock().unwrap().start();
        Ok(Async::Ready(()))
    }
}

pub fn get_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
    notifier: Arc<Mutex<Notifier>>,
) -> PadreDebugger {
    let debugger_type = match debugger_type {
        Some(s) => s.to_string(),
        None => match debugger_cmd {
            Some(s) => get_debugger_type(s).expect("Can't find debugger type, bailing"),
            None => panic!("Couldn't find debugger, try specifying with -t or -d"),
        },
    };

    let (process_tx, process_rx) = mpsc::channel(32);
    let (debugger_tx, debugger_rx) = mpsc::channel(32);

    let debugger = match debugger_type.to_ascii_lowercase().as_ref() {
        "lldb" => lldb::ImplDebugger::new(
            Arc::clone(&notifier),
            process_tx.clone(),
            debugger_rx,
            debugger_tx.clone(),
        ),
        _ => panic!("Can't build debugger type {}, panicking", &debugger_type),
    };

    tokio::spawn(debugger.map_err(|e| {
        println!("debugger error = {:?}", e);
    }));

    let debugger_arg = match debugger_cmd {
        Some(s) => s,
        None => "lldb",
    }
    .clone()
    .to_string();

    let padre_process = match debugger_type.to_ascii_lowercase().as_ref() {
        "lldb" => PadreProcess::new(Arc::new(Mutex::new(lldb::ImplProcess::new(
            Arc::clone(&notifier),
            debugger_arg,
            run_cmd,
            process_rx,
            process_tx,
            debugger_tx.clone(),
        )))),
        _ => panic!("Can't build debugger type {}, panicking", &debugger_type),
    };

    tokio::spawn(padre_process.map_err(|e| {
        println!("process error = {:?}", e);
    }));

    PadreDebugger::new(notifier, debugger_tx)
}

pub fn get_debugger_type(cmd: &str) -> Option<String> {
    //    if is_node(cmd[0]) {
    //        Some(String::from("node"))
    //    } else
    if is_lldb(&cmd) {
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
