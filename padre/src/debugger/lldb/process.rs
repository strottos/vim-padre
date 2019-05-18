//! run the lldb process and communicate with it

extern crate nix;

use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;
use std::process::exit;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use crate::debugger::ProcessTrait;
use crate::notifier::{LogLevel, Notifier};

use nix::fcntl::{open, OFlag};
use nix::libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
use nix::pty::{grantpt, posix_openpt, unlockpt, PtyMaster};
use nix::sys::stat;
use nix::unistd::{dup, dup2, execvp, fork, setsid, ForkResult};

// Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
#[cfg(target_os = "linux")]
use nix::pty::ptsname_r;

#[cfg(target_os = "macos")]
/// ptsname_r is a linux extension but ptsname isn't thread-safe
/// instead of using a static mutex this calls ioctl with TIOCPTYGNAME directly
/// based on https://blog.tarq.io/ptsname-on-osx-with-rust/
fn ptsname_r(fd: &PtyMaster) -> nix::Result<String> {
    use nix::libc::{ioctl, TIOCPTYGNAME};
    use std::ffi::CStr;

    /// the buffer size on OSX is 128, defined by sys/ttycom.h
    let buf: [i8; 128] = [0; 128];

    unsafe {
        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as u64, &buf) {
            0 => {
                let res = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                Ok(res)
            }
            _ => Err(nix::Error::last()),
        }
    }
}

#[derive(Debug)]
pub struct ImplProcess {
    notifier: Arc<Mutex<Notifier>>,
    debugger_command: String,
    run_command: Vec<String>,
    has_started: bool,
}

impl ImplProcess {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        debugger_command: String,
        run_command: Vec<String>,
    ) -> ImplProcess {
        ImplProcess {
            notifier,
            debugger_command,
            run_command,
            has_started: false,
        }
    }
}

impl ProcessTrait for ImplProcess {
    fn start(&mut self) {
        if !Path::new(&self.run_command[0]).exists() {
            self.notifier.lock().unwrap().log_msg(
                LogLevel::CRITICAL,
                format!("Can't spawn LLDB as {} does not exist", self.run_command[0]),
            );
            println!("Can't spawn LLDB as {} does not exist", self.run_command[0]);
            exit(1);
        }

        // Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
        let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();

        // Allow a slave to be generated for it
        grantpt(&master_fd).unwrap();
        unlockpt(&master_fd).unwrap();

        // on Linux this is the libc function, on OSX this is our implementation of ptsname_r
        let slave_name = ptsname_r(&master_fd).unwrap();

        match fork().unwrap() {
            ForkResult::Child => {
                setsid().unwrap(); // create new session with child as session leader
                let slave_fd = open(
                    std::path::Path::new(&slave_name),
                    OFlag::O_RDWR,
                    stat::Mode::empty(),
                )
                .unwrap();

                // assign stdin, stdout, stderr to the tty, just like a terminal does
                dup2(slave_fd, STDIN_FILENO).unwrap();
                dup2(slave_fd, STDOUT_FILENO).unwrap();
                dup2(slave_fd, STDERR_FILENO).unwrap();

                // set echo off?
                //let mut flags = termios::tcgetattr(STDIN_FILENO).unwrap();
                //flags.local_flags &= !termios::LocalFlags::ECHO;
                //termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSANOW, &flags).unwrap();

                let path = CString::new(self.debugger_command.clone()).unwrap();
                let mut argv: Vec<CString> = vec![path.clone(), CString::new("--").unwrap()];
                for arg in self.run_command.clone().into_iter() {
                    argv.push(CString::new(arg.as_bytes()).unwrap());
                }

                execvp(&path, &argv[..]).unwrap();

                exit(-1);
            }
            ForkResult::Parent { child: _child_pid } => {
                let fd = dup(master_fd.as_raw_fd()).unwrap();
                let mut process_stdio = unsafe { File::from_raw_fd(fd) };
                let mut process_out = process_stdio.try_clone().unwrap();

                let notifier_err = self.notifier.clone();

                //                let (tx, rx) = mpsc::channel();
                //
                //                thread::spawn(move || {
                //                    loop {
                //                        match process_stdio.write(rx.recv().unwrap().as_bytes()) {
                //                            Err(err) => {
                //                                notifier_err.lock()
                //                                            .unwrap()
                //                                            .log_msg(LogLevel::CRITICAL,
                //                                                     format!("Can't write to LLDB stdin: {}", err));
                //                                println!("Can't write to LLDB stdin: {}", err);
                //                                exit(1);
                //                            },
                //                            _ => {}
                //                        };
                //                    }
                //                });
                //
                //                let notifier = self.notifier.clone();
                //                let notifier_err = self.notifier.clone();
                //                let listener = self.listener.clone();
                //
                //                thread::spawn(move || {
                //                    loop {
                //                        let mut buffer: [u8; 512] = [0; 512];
                //                        match process_out.read(&mut buffer) {
                //                            Err(err) => {
                //                                notifier_err.lock()
                //                                            .unwrap()
                //                                            .log_msg(LogLevel::CRITICAL,
                //                                                     format!("Can't read from LLDB: {}", err));
                //                                println!("Can't read from LLDB: {}", err);
                //                                exit(1);
                //                            },
                //                            _ => {},
                //                        };
                //                        let data = String::from_utf8_lossy(&buffer[..]);
                //                        let data = data.trim_matches(char::from(0));
                //
                //                        print!("{}", &data);
                //
                //                        analyse_stdout(&data, &notifier, &listener);
                //                        analyse_stderr(&data, &notifier, &listener);
                //
                //                        io::stdout().flush().unwrap();
                //                    }
                //                });

                self.notifier.lock().unwrap().signal_started();
            }
        };

        //        println!("{:?}", &self.run_command);
        //        let mut process = Command::new(&self.debugger_command)
        //                                  .arg("--")
        //                                  .args(&self.run_command)
        //                                  .stdin(Stdio::piped())
        //                                  .stdout(Stdio::piped())
        //                                  .stderr(Stdio::piped())
        //                                  .spawn_async()
        //                                  .unwrap();
        //
        //        let process_stdout = process.stdout().take().unwrap();
        //        let reader = io::BufReader::new(process_stdout);
        //        let lines = tokio::io::lines(reader);
        //        let notifier = Arc::clone(&self.notifier);
        //        let stdout_process = lines.for_each(move |l| {
        //            lazy_static! {
        //                static ref RE_STOPPED_AT_POSITION: Regex = Regex::new(" *frame #\\d.*$").unwrap();
        //            }
        //
        //            for _ in RE_STOPPED_AT_POSITION.captures_iter(&l) {
        //                lazy_static! {
        //                    static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
        //                }
        //
        //                for cap in RE_JUMP_TO_POSITION.captures_iter(&l) {
        //                    notifier.lock().unwrap().jump_to_position(
        //                        cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        //                }
        //
        //                notifier.lock().unwrap().log_msg(
        //                    LogLevel::WARN, "Stopped at unknown position".to_string());
        //            }
        //
        //            println!("{}", l);
        //
        //            io::stdout().flush().unwrap();
        //
        //            Ok(())
        //        });
        //
        //        let process_stderr = process.stderr().take().unwrap();
        //        let reader = io::BufReader::new(process_stderr);
        //        let lines = tokio::io::lines(reader);
        //        let stderr_process = lines.for_each(|l| {
        //            eprintln!("{}", &l);
        //
        //            io::stderr().flush().unwrap();
        //
        //            Ok(())
        //        });
        //
        // TODO: Find out how to take from stdin and from the PADRE debugger
        //        let mut process_stdin = process.stdin().take().unwrap();
        //        let writer = io::BufWriter::new(process_stdin);
        //        let lines = tokio::io::lines(writer);
        //
        //        let (tx, mut rx) = mpsc::unbounded();
        //
        //        tx.unbounded_send("b main\n".as_bytes()).unwrap();
        //
        //        match rx.poll().unwrap() {
        //            Async::Ready(Some(v)) => {
        //                println!("HERE: {:?}", &process);
        //                println!("HERE: {:?}", v);
        ////                match process_stdin.write(v) {
        ////                    Ok(s) => println!("HERE2: {:?}", s),
        ////                    Err(err) => println!("HERE3: {:?}", err),
        ////                };
        //            },
        //            _ => {},
        //        }

        //        println!("HERE: {:?}", process);
        //
        //        let future = stdout_process
        //            .join(stderr_process)
        //            .join(process)
        //            .map(|status| {
        //                println!("Finished with status: {:?}", status);
        //            })
        //            .map_err(|e| {
        //                println!("Errored with: {:?}", e);
        //            });

        // TODO: Should be triggered by analysing stdout
        self.has_started = true;

        //        tokio::spawn(future);
    }

    fn has_started(&self) -> bool {
        self.has_started
    }

    fn stop(&self) {}
}

//use std::ffi::CString;
//use std::fs::File;
//use std::io;
//use std::io::{Read, Write};
//use std::os::unix::io::{FromRawFd, AsRawFd};
//use std::path::Path;
//use std::process::exit;
//use std::sync::{Arc, Condvar, Mutex};
//use std::sync::mpsc::{SyncSender, Receiver};
//use std::thread;
//
//use crate::notifier::{LogLevel, Notifier};
//use crate::debugger::lldb::LLDBStatus;
//
//use nix::fcntl::{OFlag, open};
//use nix::pty::{grantpt, posix_openpt, unlockpt};
//use nix::sys::stat;
//use nix::libc::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
//use nix::unistd::{fork, ForkResult, setsid, dup, dup2, execvp};
//use regex::Regex;
//
//// Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
//#[cfg(target_os = "linux")]
//use nix::pty::ptsname_r;
//
//#[cfg(target_os = "macos")]
///// ptsname_r is a linux extension but ptsname isn't thread-safe
///// instead of using a static mutex this calls ioctl with TIOCPTYGNAME directly
///// based on https://blog.tarq.io/ptsname-on-osx-with-rust/
//fn ptsname_r(fd: &PtyMaster) -> nix::Result<String> {
//    use std::ffi::CStr;
//    use std::os::unix::io::AsRawFd;
//    use nix::libc::{ioctl, TIOCPTYGNAME};
//
//    /// the buffer size on OSX is 128, defined by sys/ttycom.h
//    let buf: [i8; 128] = [0; 128];
//
//    unsafe {
//        match ioctl(fd.as_raw_fd(), TIOCPTYGNAME as u64, &buf) {
//            0 => {
//                let res = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
//                Ok(res)
//            }
//            _ => Err(nix::Error::last()),
//        }
//    }
//}
//
//#[derive(Debug)]
//pub struct LLDBProcess {
//    notifier: Arc<Mutex<Notifier>>,
//    listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
//    sender: Option<SyncSender<String>>,
//}
//
//impl LLDBProcess {
//    pub fn new(
//        notifier: Arc<Mutex<Notifier>>,
//        listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
//        sender: Option<SyncSender<String>>
//    ) -> LLDBProcess {
//        LLDBProcess {
//            notifier: notifier,
//            listener: listener,
//            sender: sender,
//        }
//    }
//
//    pub fn start_process(
//        &mut self,
//        debugger_command: String,
//        run_command: &Vec<String>,
//        receiver: Receiver<String>,
//    ) {
//        if !Path::new(&run_command[0]).exists() {
//            self.notifier
//                .lock()
//                .unwrap()
//                .log_msg(LogLevel::CRITICAL,
//                         format!("Can't spawn LLDB as {} does not exist", run_command[0]));
//            println!("Can't spawn LLDB as {} does not exist", run_command[0]);
//            exit(1);
//        }
//
//        // Code based on https://github.com/philippkeller/rexpect/blob/master/src/process.rs
//        let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();
//
//        // Allow a slave to be generated for it
//        grantpt(&master_fd).unwrap();
//        unlockpt(&master_fd).unwrap();
//
//        // on Linux this is the libc function, on OSX this is our implementation of ptsname_r
//        let slave_name = ptsname_r(&master_fd).unwrap();
//
//        match fork().unwrap() {
//            ForkResult::Child => {
//                setsid().unwrap(); // create new session with child as session leader
//                let slave_fd = open(std::path::Path::new(&slave_name),
//                                    OFlag::O_RDWR,
//                                    stat::Mode::empty()).unwrap();
//
//                // assign stdin, stdout, stderr to the tty, just like a terminal does
//                dup2(slave_fd, STDIN_FILENO).unwrap();
//                dup2(slave_fd, STDOUT_FILENO).unwrap();
//                dup2(slave_fd, STDERR_FILENO).unwrap();
//
//                // set echo off?
//                //let mut flags = termios::tcgetattr(STDIN_FILENO).unwrap();
//                //flags.local_flags &= !termios::LocalFlags::ECHO;
//                //termios::tcsetattr(STDIN_FILENO, termios::SetArg::TCSANOW, &flags).unwrap();
//
//                let path = CString::new(debugger_command).unwrap();
//                let mut argv: Vec<CString> = vec!(path.clone(), CString::new("--").unwrap());
//                for arg in run_command.into_iter() {
//                    argv.push(CString::new(arg.as_bytes()).unwrap());
//                }
//
//                execvp(&path, &argv[..]).unwrap();
//
//                exit(-1);
//            },
//            ForkResult::Parent { child: _child_pid } => {
//                let fd = dup(master_fd.as_raw_fd()).unwrap();
//                let mut process_stdio = unsafe { File::from_raw_fd(fd) };
//                let mut process_out = process_stdio.try_clone().unwrap();
//
//                let notifier = self.notifier.clone();
//                let notifier_err = self.notifier.clone();
//                let listener = self.listener.clone();
//
//                thread::spawn(move || {
//                    loop {
//                        match process_stdio.write(receiver.recv().unwrap().as_bytes()) {
//                            Err(err) => {
//                                notifier_err.lock()
//                                            .unwrap()
//                                            .log_msg(LogLevel::CRITICAL,
//                                                     format!("Can't write to LLDB stdin: {}", err));
//                                println!("Can't write to LLDB stdin: {}", err);
//                                exit(1);
//                            },
//                            _ => {}
//                        };
//                    }
//                });
//
//                let notifier_err = self.notifier.clone();
//
//                thread::spawn(move || {
//                    loop {
//                        let mut buffer: [u8; 512] = [0; 512];
//                        match process_out.read(&mut buffer) {
//                            Err(err) => {
//                                notifier_err.lock()
//                                            .unwrap()
//                                            .log_msg(LogLevel::CRITICAL,
//                                                     format!("Can't read from LLDB: {}", err));
//                                println!("Can't read from LLDB: {}", err);
//                                exit(1);
//                            },
//                            _ => {},
//                        };
//                        let data = String::from_utf8_lossy(&buffer[..]);
//                        let data = data.trim_matches(char::from(0));
//
//                        print!("{}", &data);
//
//                        analyse_stdout(&data, &notifier, &listener);
//                        analyse_stderr(&data, &notifier, &listener);
//
//                        io::stdout().flush().unwrap();
//                    }
//                });
//
//                self.notifier.lock().unwrap().signal_started();
//            }
//        };
//
//    }
//
//    pub fn add_sender(&mut self, sender: Option<SyncSender<String>>) {
//        self.sender = sender;
//    }
//}
//
//fn analyse_stdout(data: &str, notifier: &Arc<Mutex<Notifier>>,
//                  listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>) {
//    lazy_static! {
//        static ref RE_BREAKPOINT: Regex = Regex::new("Breakpoint (\\d+): where = .* at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
//        static ref RE_BREAKPOINT_PENDING: Regex = Regex::new("Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
//        static ref RE_BREAKPOINT_MULTI: Regex = Regex::new("Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
//        static ref RE_STOPPED_AT_POSITION: Regex = Regex::new(" *frame #\\d.*$").unwrap();
//        static ref RE_STEP_IN: Regex = Regex::new("\\* .* stop reason = step in$").unwrap();
//        static ref RE_STEP_OVER: Regex = Regex::new("\\* .* stop reason = step over$").unwrap();
//        static ref RE_CONTINUE: Regex = Regex::new("Process (\\d+) resuming$").unwrap();
//        static ref RE_PRINTED_VARIABLE: Regex = Regex::new("\\((.*)\\) (\\S+) = (.*)$").unwrap();
//        static ref RE_PROCESS_STARTED: Regex = Regex::new("Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
//        static ref RE_PROCESS_EXITED: Regex = Regex::new("Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
//    }
//
//    for line in data.split("\r\n") {
//        for cap in RE_BREAKPOINT.captures_iter(line) {
//            notifier.lock().unwrap().breakpoint_set(
//                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
//            send_listener(listener, LLDBStatus::Breakpoint, vec!());
//        }
//
//        for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
//            send_listener(listener, LLDBStatus::BreakpointPending, vec!());
//        }
//
//        for _ in RE_BREAKPOINT_MULTI.captures_iter(line) {
//            send_listener(listener, LLDBStatus::Breakpoint, vec!());
//        }
//
//        for _ in RE_STOPPED_AT_POSITION.captures_iter(line) {
//            analyse_stopped_output(&notifier, line);
//        }
//
//        for _ in RE_STEP_IN.captures_iter(line) {
//            send_listener(listener, LLDBStatus::StepIn, vec!());
//        }
//
//        for _ in RE_STEP_OVER.captures_iter(line) {
//            send_listener(listener, LLDBStatus::StepOver, vec!());
//        }
//
//        for _ in RE_CONTINUE.captures_iter(line) {
//            send_listener(listener, LLDBStatus::Continue, vec!());
//        }
//
//        for cap in RE_PROCESS_STARTED.captures_iter(line) {
//            let args = vec!(cap[1].to_string());
//
//            send_listener(listener, LLDBStatus::ProcessStarted, args);
//        }
//
//        for cap in RE_PROCESS_EXITED.captures_iter(line) {
//            notifier.lock().unwrap().signal_exited(
//                cap[1].parse::<u32>().unwrap(), cap[2].parse::<u8>().unwrap());
//        }
//
//        for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
//            let variable_type = cap[1].to_string();
//            let variable_name = cap[2].to_string();
//            let variable_value = get_variable_value(&data, &cap[1], &cap[2], &cap[3]);
//            let args = vec!(variable_name, variable_value, variable_type);
//
//            send_listener(listener, LLDBStatus::Variable, args);
//        }
//    }
//}
//
//// TODO: Coalesce with previous function
//fn analyse_stderr(data: &str, notifier: &Arc<Mutex<Notifier>>, listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>) {
//    lazy_static! {
//        static ref RE_PROCESS_NOT_RUNNING: Regex = Regex::new("error: invalid process$").unwrap();
//        static ref RE_VARIABLE_NOT_FOUND: Regex = Regex::new("error: no variable named '(.*)' found in this frame$").unwrap();
//        static ref RE_ERROR: Regex = Regex::new("error: (.*)$").unwrap();
//    }
//
//    let mut matched: bool = false;
//
//    for line in data.split("\r\n") {
//        for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
//            notifier.lock()
//                    .unwrap()
//                    .log_msg(LogLevel::WARN, "program not running".to_string());
//            send_listener(listener, LLDBStatus::NoProcess, vec!());
//            matched = true;
//        }
//
//        for _ in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
//            send_listener(listener, LLDBStatus::VariableNotFound, vec!());
//            matched = true;
//        }
//
//        if matched {
//            matched = false;
//            continue;
//        }
//
//        for cap in RE_ERROR.captures_iter(line) {
//            let args = vec!(cap[1].to_string());
//
//            send_listener(listener, LLDBStatus::Error, args);
//        }
//    }
//}
//
//fn analyse_stopped_output(notifier: &Arc<Mutex<Notifier>>, line: &str) {
//    lazy_static! {
//        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d at (\\S+):(\\d+)$").unwrap();
//    }
//
//    for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
//        notifier.lock().unwrap().jump_to_position(
//            cap[1].to_string(), cap[2].parse::<u32>().unwrap());
//        return
//    }
//
//    notifier.lock().unwrap().log_msg(
//        LogLevel::WARN, "Stopped at unknown position".to_string());
//}
//
//fn send_listener(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>, lldb_status: LLDBStatus, args: Vec<String>) {
//    let &(ref lock, ref cvar) = &**listener;
//    let mut started = lock.lock().unwrap();
//    *started = (lldb_status, args);
//    cvar.notify_one();
//}
//
//// TODO: Fix all these listeners, must be a way of getting these functions into the object (or an
//// object on top maybe).
//fn get_variable_value(data: &str, variable_type: &str, variable_name: &str, value: &str) -> String {
//    if get_variable_is_vector(variable_type) {
//        let x: &[_] = &['v', 'e', 'c', '!'];
//        return value.trim_start_matches(x).to_string();
//    }
//
//    value.to_string()
//}
//
//fn get_variable_is_vector(variable_type: &str) -> bool {
//    lazy_static! {
//        static ref RE_RUST_IS_VECTOR: Regex = Regex::new("^&*alloc::vec::Vec<(.*)>").unwrap();
//    }
//
//    for _ in RE_RUST_IS_VECTOR.captures_iter(variable_type) {
//        return true;
//    }
//
//    false
//}
//
//#[cfg(test)]
//mod tests {
//    #[test]
//    fn check_value_int() {
//        let ret = super::get_variable_value("(int) i = 42", "int", "i", "42");
//        assert_eq!(ret, "42".to_string());
//    }
//
//    #[test]
//    fn check_value_string() {
//        let ret = super::get_variable_value("(alloc::string::String) s = \"TESTING\"",
//                                            "alloc::string::String", "s", "TESTING");
//        assert_eq!(ret, "TESTING".to_string());
//    }
//
//    #[test]
//    fn check_value_string_ref() {
//        let ret = super::get_variable_value("(&str *) s = 0x00007ffeefbff368",
//                                            "&str *", "s", "0x00007ffeefbff368");
//        assert_eq!(ret, "0x00007ffeefbff368".to_string());
//    }
//
////    #[test]
////    fn check_value_vector_of_strings() {
////        let ret = super::get_variable_value("alloc::vec::Vec<alloc::string::String>",
////                                            "vec![\"TEST1\", \"TEST2\", \"TEST3\"]");
////        assert_eq!(ret, "[\"TEST1\", \"TEST2\", \"TEST3\"]");
////    }
////
////    #[test]
////    fn check_value_struct() {
////        let data = "(std::net::tcp::TcpListener) listener = TcpListener(TcpListener {
////inner: Socket(FileDesc {
////fd: 3
////})
////})";
////        let ret = super::get_variable_value(data, "int", "i", "42");
////        assert_eq!(ret, "42".to_string());
////    }
//}
