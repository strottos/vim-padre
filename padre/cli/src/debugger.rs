//! Handle creating the debugger dependent on the type of debugger specified
//!
//! See core/debugger.rs for more centric/shared debugger material once one is created

use std::time::Instant;

use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use padre_core::debugger::DebuggerCmd;
use padre_core::server::Notification;
use padre_core::Result;

#[cfg(feature = "lldb")]
use padre_lldb;
#[cfg(feature = "node")]
use padre_node;
#[cfg(feature = "python")]
use padre_python;

/// Debuggers
#[derive(Debug)]
pub enum DebuggerType {
    #[cfg(feature = "lldb")]
    LLDB,
    #[cfg(feature = "node")]
    Node,
    #[cfg(feature = "python")]
    Python,
}

#[derive(Debug)]
pub struct Debugger {
    debugger_type: DebuggerType,
    notifier_tx: mpsc::Sender<Notification>,
}

impl Debugger {
    /// Get the debugger implementation
    ///
    /// If the debugger type is not specified it will try it's best to guess what kind of debugger to
    /// return.
    pub fn new(debugger_type: DebuggerType, notifier_tx: mpsc::Sender<Notification>) -> Self {
        Debugger {
            debugger_type,
            notifier_tx,
        }
    }

    pub async fn run(
        &mut self,
        debugger_cmd: String,
        run_cmd: Vec<String>,
        debugger_queue_rx: mpsc::Receiver<(
            DebuggerCmd,
            Instant,
            oneshot::Sender<Result<serde_json::Value>>,
        )>,
        stop_rx: oneshot::Receiver<()>,
        stop_tx: oneshot::Sender<()>,
    ) {
        match self.debugger_type {
            #[cfg(feature = "lldb")]
            DebuggerType::LLDB => {
                let mut debugger = padre_lldb::get_debugger(
                    debugger_cmd,
                    run_cmd,
                    debugger_queue_rx,
                    self.notifier_tx.clone(),
                );

                tokio::select! {
                    _ = debugger.start() => {}
                    _ = stop_rx => {
                        debugger.stop().await;

                        stop_tx.send(()).unwrap();
                    }
                };
            }
            #[cfg(feature = "node")]
            DebuggerType::Node => {
                "node".to_string();
            }
            #[cfg(feature = "python")]
            DebuggerType::Python => {
                let mut debugger = padre_python::get_debugger(
                    debugger_cmd,
                    run_cmd,
                    debugger_queue_rx,
                    self.notifier_tx.clone(),
                );

                tokio::select! {
                    _ = debugger.start() => {}
                    _ = stop_rx => {
                        debugger.stop().await;

                        stop_tx.send(()).unwrap();
                    }
                };
            }
        };
    }
}

/// Guesses the debugger type and the debugger command if not specified
pub async fn get_debugger_info(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: &Vec<String>,
) -> (DebuggerType, String) {
    let debugger_type = get_debugger_type(debugger_cmd, debugger_type, run_cmd).await;
    let debugger_command = get_debugger_command(debugger_cmd, &debugger_type).await;

    (debugger_type, debugger_command)
}

async fn get_debugger_type(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: &Vec<String>,
) -> DebuggerType {
    match debugger_type {
        Some(s) => match s.to_ascii_lowercase().as_str() {
            #[cfg(feature = "lldb")]
            "lldb" => DebuggerType::LLDB,
            #[cfg(feature = "node")]
            "node" => DebuggerType::Node,
            #[cfg(feature = "python")]
            "python" => DebuggerType::Python,
            _ => panic!("Couldn't understand debugger type {}", s),
        },
        None => {
            #[cfg(feature = "node")]
            if is_node(&run_cmd[0]).await {
                return DebuggerType::Node;
            }

            #[cfg(feature = "python")]
            if is_python(&run_cmd[0]).await {
                return DebuggerType::Python;
            }

            #[cfg(feature = "lldb")]
            if is_lldb(&run_cmd[0]).await {
                return DebuggerType::LLDB;
            }

            match debugger_cmd {
                Some(s) => match s {
                    #[cfg(feature = "lldb")]
                    "lldb" => DebuggerType::LLDB,
                    #[cfg(feature = "node")]
                    "node" => DebuggerType::Node,
                    #[cfg(feature = "python")]
                    "python" | "python3" => DebuggerType::Python,
                    _ => panic!(
                        "Can't find debugger type for {}, try specifying with -d or -t",
                        s
                    ),
                },
                None => panic!("Can't find debugger type, try specifying with -d or -t"),
            }
        }
    }
}

async fn get_debugger_command(debugger_cmd: Option<&str>, debugger_type: &DebuggerType) -> String {
    match debugger_cmd {
        Some(s) => s.to_string(),
        None => match debugger_type {
            #[cfg(feature = "lldb")]
            DebuggerType::LLDB => "lldb".to_string(),
            #[cfg(feature = "node")]
            DebuggerType::Node => "node".to_string(),
            #[cfg(feature = "python")]
            DebuggerType::Python => "python3".to_string(),
        },
    }
}

/// Checks if the file is a binary executable
#[cfg(feature = "lldb")]
async fn is_lldb(cmd: &str) -> bool {
    if file_is_binary_executable(cmd).await {
        return true;
    }

    false
}

// /// Checks if the file is a NodeJS script
// #[cfg(feature = "node")]
// async fn is_node(cmd: &str) -> bool {
//     if file_is_text(cmd).await && cmd.ends_with(".js") {
//         return true;
//     }
//
//     // if file_is_binary_executable(cmd) && cmd.contains("node") {
//     //     return true;
//     // }
//
//     false
// }

/// Checks if the file is a Python script
#[cfg(feature = "python")]
async fn is_python(cmd: &str) -> bool {
    if file_is_text(cmd).await && cmd.ends_with(".py") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("python") {
    //     return true;
    // }

    false
}

/// Find out if a file is a binary executable (either ELF or Mach-O
/// executable).
async fn file_is_binary_executable(cmd: &str) -> bool {
    let output = get_file_type(cmd).await;

    if output.contains("ELF")
        || (output.contains("Mach-O") && output.to_ascii_lowercase().contains("executable"))
    {
        true
    } else {
        false
    }
}

/// Get the file type as output by the UNIX `file` command.
async fn get_file_type(cmd: &str) -> String {
    let output = Command::new("file")
        .arg("-L") // Follow symlinks
        .arg(cmd)
        .output();
    let output = output
        .await
        .expect(&format!("Can't run file on {} to find file type", cmd));

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Find out if a file is a text file (either ASCII or UTF-8).
async fn file_is_text(cmd: &str) -> bool {
    let output = get_file_type(cmd).await;

    if output.contains("ASCII") || output.contains("UTF-8") {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn is_file_executable() {
        assert_eq!(
            true,
            super::file_is_binary_executable("../test_files/node").await
        );
        assert_eq!(
            false,
            super::file_is_binary_executable("../test_files/test_node.js").await
        );
    }

    #[tokio::test]
    async fn is_file_text() {
        assert_eq!(false, super::file_is_text("../test_files/node").await);
        assert_eq!(
            true,
            super::file_is_text("../test_files/test_node.js").await
        );
    }
}
