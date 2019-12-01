//! Debugger Module
//!
//! Main module for handling the debuggers, defines the standard versioned debugger interfaces
//! and creates the main debugger objects.

use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use padre_core::server::DebuggerCmd;
use padre_core::util::{file_is_binary_executable, file_is_text};

use futures::StreamExt;
use tokio::sync::mpsc::Receiver;

//mod lldb;
//mod node;
#[cfg(feature = "python")]
use padre_python;

/// Debuggers
#[derive(Debug)]
enum DebuggerType {
    LLDB,
    Node,
    Python,
}

#[derive(Debug)]
pub struct Debugger {
    debugger: Arc<Mutex<padre_python::ImplDebugger>>,
}

impl Debugger {
    pub fn new(impl_debugger: Arc<Mutex<padre_python::ImplDebugger>>, mut queue_rx: Receiver<(DebuggerCmd, Instant)>) -> Debugger {
        let debugger = Debugger {
            debugger: impl_debugger.clone(),
        };

        let queue_processing_debugger = impl_debugger.clone();

        tokio::spawn(async move {
            while let Some(cmd) = queue_rx.next().await {
                let mut debugger = queue_processing_debugger.lock().unwrap();
                match cmd.0 {
                    DebuggerCmd::Run => debugger.run(cmd.1),
                    DebuggerCmd::Breakpoint(fl) => debugger.breakpoint(&fl, cmd.1),
                    DebuggerCmd::StepIn => debugger.step_in(cmd.1),
                    DebuggerCmd::StepOver => debugger.step_over(cmd.1),
                    DebuggerCmd::Continue => debugger.continue_(cmd.1),
                    DebuggerCmd::Print(v) => debugger.print(&v, cmd.1),
                };
            };
        });

        debugger
    }

    pub fn stop(&mut self) {
        self.debugger.lock().unwrap().teardown();
    }
}

/// Get the debugger implementation
///
/// If the debugger type is not specified it will try it's best to guess what kind of debugger to
/// return.
pub async fn create_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
    queue_rx: Receiver<(DebuggerCmd, Instant)>,
) -> Debugger {
    let debugger_type = match debugger_type {
        Some(s) => match s.to_ascii_lowercase().as_str() {
            "lldb" => DebuggerType::LLDB,
            "python" => DebuggerType::Python,
            "node" => DebuggerType::Node,
            _ => panic!("Couldn't understand debugger type {}", s),
        },
        None => match get_debugger_type(&run_cmd[0]).await {
            Some(s) => s,
            None => match debugger_cmd {
                Some(s) => match s {
                    "lldb" => DebuggerType::LLDB,
                    "python" | "python3" => DebuggerType::Python,
                    "node" => DebuggerType::Node,
                    _ => panic!(
                        "Can't find debugger type for {}, try specifying with -d or -t",
                        s
                    ),
                },
                None => panic!("Can't find debugger type, try specifying with -d or -t"),
            },
        },
    };

    let debugger_cmd = match debugger_cmd {
        Some(s) => s.to_string(),
        None => match debugger_type {
            DebuggerType::LLDB => "lldb".to_string(),
            DebuggerType::Node => "node".to_string(),
            DebuggerType::Python => "python3".to_string(),
        },
    };

    let debugger: padre_python::ImplDebugger = padre_python::ImplDebugger::new(debugger_cmd, run_cmd);
    //        DebuggerType::LLDB => Box::new(lldb::ImplDebugger::new(debugger_cmd, run_cmd)),
    //        DebuggerType::Node => Box::new(node::ImplDebugger::new(debugger_cmd, run_cmd)),
    //        DebuggerType::Python => Box::new(padre_python::ImplDebugger::new(debugger_cmd, run_cmd)),
    //    };

    Debugger::new(Arc::new(Mutex::new(debugger)), queue_rx)
}

/// Guesses the debugger type
async fn get_debugger_type(run_cmd: &str) -> Option<DebuggerType> {
    if is_node(&run_cmd).await {
        Some(DebuggerType::Node)
    } else if is_python(&run_cmd).await {
        Some(DebuggerType::Python)
    } else if is_lldb(&run_cmd).await {
        Some(DebuggerType::LLDB)
    } else {
        None
    }
}

/// Checks if the file is a binary executable
async fn is_lldb(cmd: &str) -> bool {
    if file_is_binary_executable(cmd).await {
        return true;
    }

    false
}

/// Checks if the file is a NodeJS script
async fn is_node(cmd: &str) -> bool {
    if file_is_text(cmd).await && cmd.ends_with(".js") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("node") {
    //     return true;
    // }

    false
}

/// Checks if the file is a Python script
async fn is_python(cmd: &str) -> bool {
    if file_is_text(cmd).await && cmd.ends_with(".py") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("python") {
    //     return true;
    // }

    false
}
