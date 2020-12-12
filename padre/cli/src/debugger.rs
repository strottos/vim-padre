//! Handle creating the debugger dependent on the type of debugger specified
//!
//! See core/debugger.rs for more centric/shared debugger material once one is created

use std::fmt::Debug;
use std::process::Command;

use padre_core::debugger::Debugger;

#[cfg(feature = "godlv")]
use padre_godlv;
#[cfg(feature = "lldb")]
use padre_lldb;
#[cfg(feature = "node")]
use padre_node;
#[cfg(feature = "python")]
use padre_python;

/// Debuggers
#[derive(Debug)]
enum DebuggerType {
    #[cfg(feature = "lldb")]
    LLDB,
    #[cfg(feature = "node")]
    Node,
    #[cfg(feature = "python")]
    Python,
    #[cfg(feature = "godlv")]
    GoDlv,
}

/// Get the debugger implementation
///
/// If the debugger type is not specified it will try it's best to guess what kind of debugger to
/// return.
pub fn create_debugger(
    debugger_cmd: Option<&str>,
    debugger_type: Option<&str>,
    run_cmd: Vec<String>,
) -> Box<dyn Debugger + Send> {
    let debugger_type = match debugger_type {
        Some(s) => match s.to_ascii_lowercase().as_str() {
            #[cfg(feature = "lldb")]
            "lldb" => DebuggerType::LLDB,
            #[cfg(feature = "python")]
            "python" => DebuggerType::Python,
            #[cfg(feature = "node")]
            "node" => DebuggerType::Node,
            #[cfg(feature = "godlv")]
            "godlv" => DebuggerType::GoDlv,
            _ => panic!("Couldn't understand debugger type {}", s),
        },
        None => match get_debugger_type(&run_cmd[0]) {
            Some(s) => s,
            None => match debugger_cmd {
                Some(s) => match s {
                    #[cfg(feature = "lldb")]
                    "lldb" => DebuggerType::LLDB,
                    #[cfg(feature = "python")]
                    "python" | "python3" => DebuggerType::Python,
                    #[cfg(feature = "node")]
                    "node" => DebuggerType::Node,
                    #[cfg(feature = "godlv")]
                    "godlv" => DebuggerType::GoDlv,
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
            #[cfg(feature = "lldb")]
            DebuggerType::LLDB => "lldb".to_string(),
            #[cfg(feature = "node")]
            DebuggerType::Node => "node".to_string(),
            #[cfg(feature = "python")]
            DebuggerType::Python => "python3".to_string(),
            #[cfg(feature = "godlv")]
            DebuggerType::GoDlv => "godlv".to_string(),
        },
    };

    match debugger_type {
        #[cfg(feature = "lldb")]
        DebuggerType::LLDB => Box::new(padre_lldb::ImplDebugger::new(debugger_cmd, run_cmd)),
        #[cfg(feature = "node")]
        DebuggerType::Node => Box::new(padre_node::ImplDebugger::new(debugger_cmd, run_cmd)),
        #[cfg(feature = "python")]
        DebuggerType::Python => Box::new(padre_python::ImplDebugger::new(debugger_cmd, run_cmd)),
        #[cfg(feature = "godlv")]
        DebuggerType::GoDlv => Box::new(padre_godlv::ImplDebugger::new(debugger_cmd, run_cmd)),
    }
}

/// Guesses the debugger type
fn get_debugger_type(run_cmd: &str) -> Option<DebuggerType> {
    if is_node(&run_cmd) {
        #[cfg(feature = "node")]
        return Some(DebuggerType::Node);
    } else if is_python(&run_cmd) {
        #[cfg(feature = "python")]
        return Some(DebuggerType::Python);
    } else if is_lldb(&run_cmd) {
        #[cfg(feature = "lldb")]
        return Some(DebuggerType::LLDB);
    }

    None
}

/// Checks if the file is a binary executable
fn is_lldb(cmd: &str) -> bool {
    if file_is_binary_executable(cmd) {
        return true;
    }

    false
}

/// Checks if the file is a NodeJS script
fn is_node(cmd: &str) -> bool {
    if file_is_text(cmd) && cmd.ends_with(".js") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("node") {
    //     return true;
    // }

    false
}

/// Checks if the file is a Python script
fn is_python(cmd: &str) -> bool {
    if file_is_text(cmd) && cmd.ends_with(".py") {
        return true;
    }

    // if file_is_binary_executable(cmd) && cmd.contains("python") {
    //     return true;
    // }

    false
}

/// Find out if a file is a binary executable (either ELF or Mach-O
/// executable).
fn file_is_binary_executable(cmd: &str) -> bool {
    let output = get_file_type(cmd);

    if output.contains("ELF")
        || (output.contains("Mach-O") && output.to_ascii_lowercase().contains("executable"))
    {
        true
    } else {
        false
    }
}

/// Find out if a file is a text file (either ASCII or UTF-8).
fn file_is_text(cmd: &str) -> bool {
    let output = get_file_type(cmd);

    if output.contains("ASCII") || output.contains("UTF-8") {
        true
    } else {
        false
    }
}

/// Get the file type as output by the UNIX `file` command.
fn get_file_type(cmd: &str) -> String {
    let output = Command::new("file")
        .arg("-L") // Follow symlinks
        .arg(cmd)
        .output();
    let output = output.expect(&format!("Can't run file on {} to find file type", cmd));

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_file_executable() {
        assert_eq!(true, super::file_is_binary_executable("../test_files/node"));
        assert_eq!(
            false,
            super::file_is_binary_executable("../test_files/test_node.js")
        );
    }

    #[test]
    fn is_file_text() {
        assert_eq!(false, super::file_is_text("../test_files/node"));
        assert_eq!(true, super::file_is_text("../test_files/test_node.js"));
    }
}
