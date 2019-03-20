//! lldb client process

use std::io;
use std::io::{Read, Write};
use std::thread;
use std::path::Path;
use std::process::{Command, exit, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::sync::mpsc::{SyncSender, Receiver};
use std::time::Duration;

use crate::notifier::{LogLevel, Notifier};
use crate::debugger::lldb::LLDBStatus;

use regex::Regex;

const TIMEOUT: u64 = 5000;

#[cfg(test)]
mod tests {
    #[test]
    fn check_type_number_int() {
        assert_eq!(super::get_variable_type("int"), "number");
    }

    #[test]
    fn check_type_number_int_ref() {
        assert_eq!(super::get_variable_type("&int *"), "JSON");
    }

    #[test]
    fn check_type_number_unsigned_int() {
        assert_eq!(super::get_variable_type("unsigned int"), "number");
    }

    #[test]
    fn check_type_number_short() {
        assert_eq!(super::get_variable_type("short"), "number");
    }

    #[test]
    fn check_type_number_unsigned_short() {
        assert_eq!(super::get_variable_type("unsigned short"), "number");
    }

    #[test]
    fn check_type_number_long() {
        assert_eq!(super::get_variable_type("long"), "number");
    }

    #[test]
    fn check_type_number_unsigned_long() {
        assert_eq!(super::get_variable_type("unsigned long"), "number");
    }

    #[test]
    fn check_type_number_long_long() {
        assert_eq!(super::get_variable_type("__int128"), "number");
    }

    #[test]
    fn check_type_number_unsigned_long_long() {
        assert_eq!(super::get_variable_type("unsigned __int128"), "number");
    }

    #[test]
    fn check_type_number_isize() {
        assert_eq!(super::get_variable_type("isize"), "number");
    }

    #[test]
    fn check_type_number_usize() {
        assert_eq!(super::get_variable_type("usize"), "number");
    }

    #[test]
    fn check_type_number_float() {
        assert_eq!(super::get_variable_type("float"), "number");
    }

    #[test]
    fn check_type_number_double() {
        assert_eq!(super::get_variable_type("double"), "number");
    }

    #[test]
    fn check_type_boolean() {
        assert_eq!(super::get_variable_type("bool"), "boolean");
    }

    #[test]
    fn check_type_string_str() {
        assert_eq!(super::get_variable_type("str"), "string");
    }

    #[test]
    fn check_type_string_str_ref() {
        assert_eq!(super::get_variable_type("&str *"), "JSON");
    }

    #[test]
    fn check_type_string_string() {
        assert_eq!(super::get_variable_type("alloc::string::String"), "string");
    }

    #[test]
    fn check_type_string_string_ref() {
        assert_eq!(super::get_variable_type("&alloc::string::String"), "string");
    }

    #[test]
    fn check_type_vector_of_strings() {
        assert_eq!(super::get_variable_type("alloc::vec::Vec<alloc::string::String>"), "JSON");
    }

//    #[test]
//    fn check_value_int() {
//        let ret = super::get_variable_value("int", "42");
//        assert_eq!(ret, "42".to_string());
//    }
//
//    #[test]
//    fn check_value_string() {
//        let ret = super::get_variable_value("alloc::string::String", "TESTING");
//        assert_eq!(ret, "TESTING".to_string());
//    }
//
//    #[test]
//    fn check_value_string_ref() {
//        let ret = super::get_variable_value("&str *", "&0x7ffeefbff2a8");
//        assert_eq!(ret, "&0x7ffeefbff2a8".to_string());
//    }
//
//    #[test]
//    fn check_value_vector_of_strings() {
//        let ret = super::get_variable_value("alloc::vec::Vec<alloc::string::String>",
//                                            "vec![\"TEST1\", \"TEST2\", \"TEST3\"]");
//        assert_eq!(ret, "[\"TEST1\", \"TEST2\", \"TEST3\"]");
//    }
}

pub struct LLDBProcess {
    notifier: Arc<Mutex<Notifier>>,
    listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
    sender: Option<SyncSender<String>>,
}

impl LLDBProcess {
    pub fn new(
        notifier: Arc<Mutex<Notifier>>,
        listener: Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
        sender: Option<SyncSender<String>>
    ) -> LLDBProcess {
        LLDBProcess {
            notifier: notifier,
            listener: listener,
            sender: sender,
        }
    }

    pub fn start_process(
        &mut self,
        debugger_command: String,
        run_command: &Vec<String>,
        receiver: Receiver<String>,
    ) {
        if !Path::new(&run_command[0]).exists() {
            self.notifier
                .lock()
                .unwrap()
                .log_msg(LogLevel::CRITICAL,
                         format!("Can't spawn LLDB as {} does not exist", run_command[0]));
            println!("Can't spawn LLDB as {} does not exist", run_command[0]);
            exit(1);
        }

        let process = Command::new(debugger_command)
                              .args(run_command)
                              .stdin(Stdio::piped())
                              .stdout(Stdio::piped())
                              .stderr(Stdio::piped())
                              .spawn()
                              .unwrap();

        let mut process_stdout = process.stdout;
        let notifier = self.notifier.clone();
        let notifier_err = self.notifier.clone();
        let listener = self.listener.clone();
        let sender = self.sender.clone();

        thread::spawn(move || {
            match process_stdout.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        match out.read(&mut buffer) {
                            Err(err) => {
                                notifier_err.lock()
                                            .unwrap()
                                            .log_msg(LogLevel::CRITICAL,
                                                     format!("Can't read from LLDB: {}", err));
                                println!("Can't read from LLDB: {}", err);
                                exit(1);
                            },
                            _ => {},
                        };
                        let data = String::from_utf8_lossy(&buffer[..]);
                        let data = data.trim_matches(char::from(0));

                        print!("{}", &data);

                        analyse_stdout(data, &notifier, &listener, &sender);

                        io::stdout().flush().unwrap();
                    }
                }
                None => {
                    notifier_err.lock()
                                .unwrap()
                                .log_msg(LogLevel::CRITICAL,
                                         "Can't read from LLDB".to_string());
                    println!("Can't read from LLDB");
                    exit(1);
                }
            }
        });

        let mut process_stderr = process.stderr;
        let notifier = self.notifier.clone();
        let notifier_err = self.notifier.clone();
        let listener = self.listener.clone();

        thread::spawn(move || {
            match process_stderr.as_mut() {
                Some(out) => {
                    loop {
                        let mut buffer: [u8; 512] = [0; 512];
                        match out.read(&mut buffer) {
                            Err(err) => {
                                notifier_err.lock()
                                            .unwrap()
                                            .log_msg(LogLevel::CRITICAL,
                                                     format!("Can't read from LLDB stderr: {}", err));
                                println!("Can't read from LLDB stderr: {}", err);
                                exit(1);
                            },
                            _ => {},
                        };
                        let data = String::from_utf8_lossy(&buffer[..]);
                        let data = data.trim_matches(char::from(0));

                        eprint!("{}", &data);

                        analyse_stderr(data, &notifier, &listener);

                        io::stdout().flush().unwrap();
                    }
                }
                None => {
                    notifier_err.lock()
                                .unwrap()
                                .log_msg(LogLevel::CRITICAL,
                                         "Can't read from LLDB stderr".to_string());
                    println!("Can't read from LLDB stderr");
                    exit(1);
                }
            }
        });

        let mut process_stdin = process.stdin;
        let notifier_err = self.notifier.clone();

        thread::spawn(move || {
            loop {
                match process_stdin.as_mut().unwrap().write(receiver.recv().unwrap().as_bytes()) {
                    Err(err) => {
                        notifier_err.lock()
                                    .unwrap()
                                    .log_msg(LogLevel::CRITICAL,
                                             format!("Can't read from PADRE stdin: {}", err));
                        println!("Can't read from PADRE stdin: {}", err);
                        exit(1);
                    },
                    _ => {}
                };
            }
        });
    }

    pub fn add_sender(&mut self, sender: Option<SyncSender<String>>) {
        self.sender = sender;
    }
}

fn analyse_stdout(data: &str, notifier: &Arc<Mutex<Notifier>>,
                  listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
                  sender: &Option<SyncSender<String>>) {
    lazy_static! {
        static ref RE_BREAKPOINT: Regex = Regex::new("^Breakpoint (\\d+): where = \\S+`\\S+ \\+ \\d+ at (\\S+):(\\d+), address = 0x[0-9a-f]*$").unwrap();
        static ref RE_BREAKPOINT_PENDING: Regex = Regex::new("^Breakpoint (\\d+): no locations \\(pending\\)\\.$").unwrap();
        static ref RE_BREAKPOINT_MULTI: Regex = Regex::new("^Breakpoint (\\d+): (\\d+) locations\\.$").unwrap();
        static ref RE_STOPPED_AT_UNKNOWN_POSITION: Regex = Regex::new("^ *frame #\\d: \\S+`(.*)$").unwrap();
        static ref RE_STEP_IN: Regex = Regex::new("^\\* .* stop reason = step in$").unwrap();
        static ref RE_STEP_OVER: Regex = Regex::new("^\\* .* stop reason = step over$").unwrap();
        static ref RE_CONTINUE: Regex = Regex::new("^Process (\\d+) resuming$").unwrap();
        static ref RE_PRINTED_VARIABLE: Regex = Regex::new("^\\((.*)\\) (\\S+) = (.*)$").unwrap();
        static ref RE_PROCESS_STARTED: Regex = Regex::new("^Process (\\d+) launched: '.*' \\((.*)\\)$").unwrap();
        static ref RE_PROCESS_EXITED: Regex = Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
        static ref RE_PROCESS_NOT_RUNNING: Regex = Regex::new("^Process (\\d+) exited with status = (\\d+) \\(0x[0-9a-f]*\\) *$").unwrap();
    }

    for line in data.split("\n") {
        for cap in RE_BREAKPOINT.captures_iter(line) {
            notifier.lock().unwrap().breakpoint_set(
                cap[2].to_string(), cap[3].parse::<u32>().unwrap());
            send_listener(listener, LLDBStatus::Breakpoint, vec!());
        }

        for _ in RE_BREAKPOINT_PENDING.captures_iter(line) {
            send_listener(listener, LLDBStatus::BreakpointPending, vec!());
        }

        for _ in RE_BREAKPOINT_MULTI.captures_iter(line) {
            send_listener(listener, LLDBStatus::Breakpoint, vec!());
        }

        for _ in RE_STOPPED_AT_UNKNOWN_POSITION.captures_iter(line) {
            analyse_stopped_output(&notifier, line);
        }

        for _ in RE_STEP_IN.captures_iter(line) {
            send_listener(listener, LLDBStatus::StepIn, vec!());
        }

        for _ in RE_STEP_OVER.captures_iter(line) {
            send_listener(listener, LLDBStatus::StepOver, vec!());
        }

        for _ in RE_CONTINUE.captures_iter(line) {
            send_listener(listener, LLDBStatus::Continue, vec!());
        }

        for cap in RE_PROCESS_STARTED.captures_iter(line) {
            let args = vec!(cap[1].to_string());

            send_listener(listener, LLDBStatus::ProcessStarted, args);
        }

        for cap in RE_PROCESS_EXITED.captures_iter(line) {
            notifier.lock().unwrap().signal_exited(
                cap[1].parse::<u32>().unwrap(), cap[2].parse::<u8>().unwrap());
        }

        for cap in RE_PRINTED_VARIABLE.captures_iter(line) {
            let variable_type = get_variable_type(&cap[1]);
            if variable_type == "" {
                panic!("Code this: {}", line);
            }
            let variable_name = cap[2].to_string();
            let variable_value = get_variable_value(listener, sender, notifier, &cap[1], &cap[2], &cap[3]);
            let args = vec!(variable_name, variable_value, variable_type);

            send_listener(listener, LLDBStatus::Variable, args);
        }
    }
}

fn analyse_stderr(data: &str, notifier: &Arc<Mutex<Notifier>>, listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>) {
    lazy_static! {
        static ref RE_PROCESS_NOT_RUNNING: Regex = Regex::new("^error: invalid process$").unwrap();
        static ref RE_VARIABLE_NOT_FOUND: Regex = Regex::new("^error: no variable named 'a' found in this frame$").unwrap();
    }

    for line in data.split("\n") {
        for _ in RE_PROCESS_NOT_RUNNING.captures_iter(line) {
            notifier.lock()
                    .unwrap()
                    .log_msg(LogLevel::WARN, "program not running".to_string());
            send_listener(listener, LLDBStatus::NoProcess, vec!());
        }

        for _ in RE_VARIABLE_NOT_FOUND.captures_iter(line) {
            send_listener(listener, LLDBStatus::VariableNotFound, vec!());
        }
    }
}

fn analyse_stopped_output(notifier: &Arc<Mutex<Notifier>>, line: &str) {
    lazy_static! {
        static ref RE_JUMP_TO_POSITION: Regex = Regex::new("^ *frame #\\d: \\S+`\\S.* at (\\S+):(\\d+)$").unwrap();
    }

    for cap in RE_JUMP_TO_POSITION.captures_iter(line) {
        notifier.lock().unwrap().jump_to_position(
            cap[1].to_string(), cap[2].parse::<u32>().unwrap());
        return
    }

    notifier.lock().unwrap().log_msg(
        LogLevel::WARN, "Stopped at unknown position".to_string());
}

fn send_listener(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>, lldb_status: LLDBStatus, args: Vec<String>) {
    let &(ref lock, ref cvar) = &**listener;
    let mut started = lock.lock().unwrap();
    *started = (lldb_status, args);
    cvar.notify_one();
}

fn get_variable_type(variable_type: &str) -> String {
    lazy_static! {
        static ref RE_RUST_IS_STRING: Regex = Regex::new("^str$|^&?alloc::string::String").unwrap();
        static ref RE_RUST_IS_NUMBER: Regex = Regex::new("^(unsigned )?(int|short|long|__int128)|^isize$|^usize$|^float$|^double$").unwrap();
        static ref RE_RUST_IS_BOOLEAN: Regex = Regex::new("^bool$").unwrap();
        static ref RE_RUST_IS_POINTER_OR_REFERENCE: Regex = Regex::new("^&*.* \\*$").unwrap();
    }

    if get_variable_is_vector(variable_type) {
        return "JSON".to_string();
    }

    for _ in RE_RUST_IS_POINTER_OR_REFERENCE.captures_iter(variable_type) {
        // To be handled above
        return variable_type.to_string();
    }

    for _ in RE_RUST_IS_STRING.captures_iter(variable_type) {
        return "string".to_string();
    }

    for _ in RE_RUST_IS_NUMBER.captures_iter(variable_type) {
        return "number".to_string();
    }

    for _ in RE_RUST_IS_BOOLEAN.captures_iter(variable_type) {
        return "boolean".to_string();
    }

    "".to_string()
}

// TODO: Fix all these listeners, must be a way of getting these functions into the object (or an
// object on top maybe).
fn get_variable_value(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
                      sender: &Option<SyncSender<String>>,
                      notifier: &Arc<Mutex<Notifier>>,
                      variable_type: &str, variable_name: &str, value: &str) -> String {
    if get_variable_is_vector(variable_type) {
        let x: &[_] = &['v', 'e', 'c', '!'];
        return value.trim_start_matches(x).to_string();
    }

//    if get_variable_is_pointer_or_reference(variable_type) {
//        let (status, args) = check_response(listener, sender, notifier,
//                                            format!("frame variable *{}", variable_name));
//        println!("{:?} {:?}", status, args);
//    }

    value.to_string()
}

fn get_variable_is_pointer_or_reference(variable_type: &str) -> bool {
    lazy_static! {
        static ref RE_RUST_IS_POINTER_OR_REFERENCE: Regex = Regex::new("^&*.* \\*$").unwrap();
    }

    for _ in RE_RUST_IS_POINTER_OR_REFERENCE.captures_iter(variable_type) {
        return true;
    }

    false
}

fn get_variable_is_vector(variable_type: &str) -> bool {
    lazy_static! {
        static ref RE_RUST_IS_VECTOR: Regex = Regex::new("^&*alloc::vec::Vec<(.*)>").unwrap();
    }

    for _ in RE_RUST_IS_VECTOR.captures_iter(variable_type) {
        return true;
    }

    false
}

// TODO: Move everything into an object here.
fn check_response(listener: &Arc<(Mutex<(LLDBStatus, Vec<String>)>, Condvar)>,
                  sender: &Option<SyncSender<String>>,
                  notifier: &Arc<Mutex<Notifier>>,
                  msg: String) -> (LLDBStatus, Vec<String>) {
    // Reset the current status
    let &(ref lock, ref cvar) = &**listener;
    let mut started = lock.lock().unwrap();
    *started = (LLDBStatus::None, vec!());

    // Send the request
    sender.clone().unwrap().send(msg.clone()).expect("Can't communicate with LLDB");

    // Check for the status change
    let result = cvar.wait_timeout(started, Duration::from_millis(TIMEOUT)).unwrap();
    started = result.0;

    match started.0 {
        LLDBStatus::None => {
            notifier.lock()
                    .unwrap()
                    .log_msg(LogLevel::CRITICAL,
                             format!("Timed out waiting for condition: {}", &msg));
            println!("Timed out waiting for condition: {}", msg);
            exit(1);
        },
        _ => {},
    };

    let status = started.0.clone();
    let args = started.1.clone();

    (status, args)
}
