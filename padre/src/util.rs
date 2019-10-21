//! Utilities
//!
//! Various simple utilities for use in PADRE

use std::env;
use std::io::{self, BufRead};
use std::mem;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{exit, Stdio};
use std::thread;

use crate::notifier::{log_msg, LogLevel};

use bytes::Bytes;
use tokio::io::AsyncRead;
use tokio::net::process::{Child, ChildStdin, Command};
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};

const BUFSIZE: usize = 4096;

/// Get an unused port on the local system and return it. This port
/// can subsequently be used.
pub fn get_unused_localhost_port() -> u16 {
    let listener = TcpListener::bind(format!("127.0.0.1:0")).unwrap();
    listener.local_addr().unwrap().port()
}

/// Log an error and a debug message, commonly used in the code base
pub fn send_error_and_debug(err_msg: &str, debug_msg: &str) {
    log_msg(LogLevel::ERROR, err_msg);
    log_msg(LogLevel::DEBUG, debug_msg);
}

/// Check whether the specified debugger and program to debug exist, including change them to
/// be the full path name if required. If it still can't find both it will panic, otherwise it
/// will start a Child process for running the program.
pub fn check_and_spawn_process(mut debugger_cmd: Vec<String>, run_cmd: Vec<String>) -> Child {
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
        let msg = format!("Can't spawn debugger as {} does not exist", s);
        log_msg(LogLevel::CRITICAL, &msg);
        println!("{}", msg);

        exit(1);
    }

    let mut args = vec![];

    for arg in &debugger_cmd[1..] {
        args.push(&arg[..]);
    }

    args.push("--");

    for arg in &run_cmd {
        args.push(&arg[..]);
    }

    Command::new(&debugger_cmd[0])
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn debugger")
}

/// Perform setup of listening and forwarding of stdin and return a sender that will forward to the
/// stdin of a process.
//pub fn setup_stdin(mut stdin: ChildStdin, output_stdin: bool) -> Sender<Bytes> {
//    let (stdin_tx, stdin_rx) = mpsc::channel(1);
//    let mut tx = stdin_tx.clone();
//
//    thread::spawn(move || {
//        let mut stdin = io::stdin();
//        loop {
//            let mut buf = vec![0; 1024];
//            let n = match stdin.read(&mut buf) {
//                Err(_) | Ok(0) => break,
//                Ok(n) => n,
//            };
//            buf.truncate(n);
//            tx = match tx.send(Bytes::from(buf)).wait() {
//                Ok(tx) => tx,
//                Err(_) => break,
//            };
//        }
//    });
//
//    // Current implementation needs a kick, this is all liable to change with
//    // upcoming versions of tokio anyway so living with it for now.
//    match stdin.write(&[13]) {
//        Err(ref e) if e.kind() == ::std::io::ErrorKind::WouldBlock => {}
//        _ => unreachable!(),
//    }
//
//    tokio::spawn(
//        stdin_rx
//            .for_each(move |text| {
//                if output_stdin {
//                    io::stdout().write_all(&text).unwrap();
//                }
//                match stdin.write(&text) {
//                    Ok(_) => {}
//                    Err(e) => {
//                        eprintln!("Writing stdin err e: {}", e);
//                    }
//                };
//                Ok(())
//            })
//            .map_err(|e| {
//                eprintln!("Reading stdin error {:?}", e);
//            }),
//    );
//
//    stdin_tx
//}

/// Find out if a file is a binary executable (either ELF or Mach-O
/// executable).
pub async fn file_is_binary_executable(cmd: &str) -> bool {
    let output = get_file_type(cmd).await;

    if output.contains("ELF")
        || (output.contains("Mach-O") && output.to_ascii_lowercase().contains("executable"))
    {
        true
    } else {
        false
    }
}

/// Find out if a file is a text file (either ASCII or UTF-8).
pub async fn file_is_text(cmd: &str) -> bool {
    let output = get_file_type(cmd).await;

    if output.contains("ASCII") || output.contains("UTF-8") {
        true
    } else {
        false
    }
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
    if !Path::new(path).exists() {
        false
    } else {
        true
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

// The following largely taken from tokio::io::lines code.

/// Combinator created by `read_output` method which is a stream over text on an I/O object.
#[derive(Debug)]
pub struct ReadOutput<A> {
    io: A,
    text: String,
}

/// Creates a new stream from the I/O object
///
/// This method takes an asynchronous I/O object, `a`, and returns a `Stream` of text that the
/// object contains. The returned stream will reach its end once `a` reaches EOF.
pub fn read_output<A>(a: A) -> ReadOutput<A>
where
    A: AsyncRead + BufRead,
{
    ReadOutput {
        io: a,
        text: String::new(),
    }
}

//impl<A> Stream for ReadOutput<A>
//where
//    A: AsyncRead + BufRead,
//{
//    type Item = String;
//    type Error = io::Error;
//
//    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
//        let mut buf = [0; BUFSIZE];
//        loop {
//            let n = match self.io.read(&mut buf) {
//                Ok(t) => t,
//                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
//                    return Ok(Async::NotReady);
//                }
//                Err(e) => return Err(e.into()),
//            };
//
//            if n == 0 && self.text.len() == 0 {
//                return Ok(None.into());
//            }
//
//            if n == BUFSIZE {
//                let bufstr = String::from_utf8_lossy(&buf[0..n]);
//                self.text.push_str(&bufstr);
//                continue;
//            }
//
//            let bufstr = String::from_utf8_lossy(&buf[0..n]);
//            self.text.push_str(&bufstr);
//            break;
//        }
//        Ok(Some(mem::replace(&mut self.text, String::new())).into())
//    }
//}

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

    #[test]
    fn is_file_executable() {
        assert_eq!(true, super::file_is_binary_executable("./test_files/node"));
        assert_eq!(
            false,
            super::file_is_binary_executable("./test_files/test_node.js")
        );
    }

    #[test]
    fn is_file_text() {
        assert_eq!(false, super::file_is_text("./test_files/node"));
        assert_eq!(true, super::file_is_text("./test_files/test_node.js"));
    }

    #[test]
    fn test_file_exists() {
        assert_eq!(true, super::file_exists("./test_files/node"));
    }

    #[test]
    fn test_file_not_exists() {
        assert_eq!(false, super::file_exists("./test_files/not_exists"));
    }

    #[test]
    fn test_getting_files_full_path_when_not_exists() {
        assert_eq!(
            "file_surely_doesnt_exist".to_string(),
            super::get_file_full_path("file_surely_doesnt_exist")
        );
    }
}
