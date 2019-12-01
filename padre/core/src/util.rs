//! Utilities
//!
//! Various simple utilities for use in PADRE

use std::env;
use std::io::{self, Write};
use std::mem;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{exit, Stdio};
use std::task::{Context, Poll};

use crate::notifier::{log_msg, LogLevel};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use pin_project::{pin_project, project};
use tokio::io::{stdin, AsyncBufRead};
use tokio::process::{Child, ChildStdin, Command};
use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};
use tokio_util::codec::{FramedRead, BytesCodec};

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
pub fn setup_stdin(mut child_stdin: ChildStdin, output_stdin: bool) -> Sender<Bytes> {
    let (stdin_tx, mut stdin_rx) = mpsc::channel(1);
    let mut tx = stdin_tx.clone();

    tokio::spawn(async move {
        let tokio_stdin = stdin();
        let mut reader = FramedRead::new(tokio_stdin, BytesCodec::new());
        while let Some(line) = reader.next().await {
            let buf = line.unwrap().freeze();
            tx.send(buf).await.unwrap();
        }
    });

    tokio::spawn(async move {
        while let Some(text) = stdin_rx.next().await {
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
    Path::new(path).exists()
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
#[pin_project]
#[derive(Debug)]
pub struct ReadOutput<R> {
    #[pin]
    reader: R,
    buf: Vec<u8>
}

/// Creates a new stream from the I/O object
///
/// This method takes an asynchronous I/O object, `reader`, and returns a `Stream` of text that
/// the object contains. The returned stream will reach its end once `reader` reaches EOF.
pub fn read_output<R>(reader: R) -> ReadOutput<R>
where
    R: AsyncBufRead,
{
    ReadOutput {
        reader,
        buf: Vec::new(),
    }
}

impl<R: AsyncBufRead> Stream for ReadOutput<R> {
    type Item = io::Result<String>;

    #[project]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        #[project]
        let ReadOutput {
            mut reader,
            buf,
        } = self.project();

        loop {
            let used = {
                match reader.as_mut().poll_fill_buf(cx) {
                    Poll::Ready(s) => {
                        match s {
                            Ok(t) => {
                                buf.extend_from_slice(t);
                                t.len()
                            }
                            Err(e) => {
                                println!("What to do here? {:?}", e);
                                0
                            }
                        }
                    }
                    Poll::Pending => {
                        0
                    }
                }
            };

            if used == 0 {
                break;
            }

            reader.as_mut().consume(used);
        }

        if buf.len() == 0 {
            return Poll::Pending;
        }

        let buf_freeze = mem::replace(buf, Vec::new());
        Poll::Ready(Some(Ok(String::from_utf8(buf_freeze).unwrap())))
    }
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

    //#[test]
    //fn is_file_executable() {
    //    tokio::spawn(async move {
    //        assert_eq!(true, super::file_is_binary_executable("./test_files/node").await);
    //        assert_eq!(
    //            false,
    //            super::file_is_binary_executable("./test_files/test_node.js").await
    //        );
    //    });
    //}

    //#[test]
    //fn is_file_text() {
    //    tokio::spawn(async move {
    //        assert_eq!(false, super::file_is_text("./test_files/node").await);
    //        assert_eq!(true, super::file_is_text("./test_files/test_node.js").await);
    //    });
    //}
}
