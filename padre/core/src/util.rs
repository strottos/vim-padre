//! Utilities
//!
//! Various simple utilities for use in PADRE

use std::env;
use std::io::{self, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use bytes::Bytes;
use futures::prelude::*;
use tokio::io::{stdin, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::server::{LogLevel, Notification, PadreError, PadreErrorKind};
use crate::Result;

/// Get an unused port on the local system and return it. This port
/// can subsequently be used.
pub fn get_unused_localhost_port() -> u16 {
    let listener = TcpListener::bind(format!("127.0.0.1:0")).unwrap();
    listener.local_addr().unwrap().port()
}

/// Check whether the specified debugger and program to debug exist, including change them to
/// be the full path name if required. If it still can't find both it will panic, otherwise it
/// will start a Child process for running the program.
pub fn check_and_spawn_process(
    mut debugger_cmd: Vec<String>,
    run_cmd: Vec<String>,
) -> Result<Child> {
    let mut not_found = None;

    // Try getting the full path if the debugger doesn't exist
    if !file_exists(&debugger_cmd[0]) {
        debugger_cmd[0] = get_file_full_path(&debugger_cmd[0]);
    }

    // Now check the debugger and program to debug exist, if not error
    if !file_exists(&run_cmd[0]) {
        not_found = Some(&run_cmd[0]);
    };

    if !file_exists(&debugger_cmd[0]) {
        not_found = Some(&debugger_cmd[0]);
    }

    if let Some(s) = not_found {
        return Err(PadreError::new(
            PadreErrorKind::ProcessSpawnError,
            "Can't spawn, debugger doesn't exist".to_string(),
            format!("Can't spawn debugger as {} does not exist", s),
        ));
    }

    let mut args = vec![];

    for arg in &debugger_cmd[0..] {
        args.push(&arg[..]);
    }

    args.push("--");

    for arg in &run_cmd {
        args.push(&arg[..]);
    }

    let mut pty_wrapper = env::current_exe().unwrap();
    pty_wrapper.pop();
    pty_wrapper.pop();
    pty_wrapper.pop();
    pty_wrapper.push("ptywrapper.py");

    Ok(Command::new(pty_wrapper)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn debugger"))
}

/// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
/// stdin of a process.
pub fn setup_stdin(mut child_stdin: ChildStdin, output_stdin: bool) -> Sender<Bytes> {
    let (stdin_tx, mut stdin_rx) = mpsc::channel(1);
    let tx = stdin_tx.clone();

    tokio::spawn(async move {
        let tokio_stdin = stdin();
        let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
        while let Some(line) = reader.next().await {
            let buf = line.unwrap().freeze();
            tx.send(buf).await.unwrap();
        }
    });

    tokio::spawn(async move {
        while let Some(text) = stdin_rx.recv().await {
            if output_stdin {
                io::stdout().write_all(&text).unwrap();
            }
            match child_stdin.write(&text).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Writing stdin err e: {}", e);
                }
            };
        }
    });

    stdin_tx
}

/// Find out the full path of a file based on the PATH environment variable.
pub fn get_file_full_path(cmd: &str) -> String {
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

/// Return true if the path specified exists.
pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Send a message to all listeners
pub fn log_msg(notifier_tx: Sender<Notification>, level: LogLevel, msg: &str) {
    let msg = Notification::new(
        "padre#debugger#Log".to_string(),
        vec![serde_json::json!(level as u8), serde_json::json!(msg)],
    );
    tokio::spawn(async move {
        notifier_tx.send(msg).await.unwrap();
    });
}

/// Notify about a code position change
pub fn jump_to_position(notifier_tx: Sender<Notification>, file: &str, line: u64) {
    let msg = Notification::new(
        "padre#debugger#JumpToPosition".to_string(),
        vec![serde_json::json!(file), serde_json::json!(line)],
    );
    tokio::spawn(async move {
        notifier_tx.send(msg).await.unwrap();
    });
}

pub fn serde_json_merge(a: &mut serde_json::Value, b: serde_json::Value) {
    if let serde_json::Value::Object(a) = a {
        if let serde_json::Value::Object(b) = b {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                } else {
                    serde_json_merge(a.entry(k).or_insert(serde_json::Value::Null), v);
                }
            }

            return;
        }
    }

    *a = b;
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn find_and_use_unused_port() {
        let port = super::get_unused_localhost_port();
        thread::sleep(Duration::new(1, 0));
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }
}
