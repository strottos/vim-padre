use std::io;
use std::net::{TcpListener};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

#[macro_use] extern crate lazy_static;
extern crate regex;
extern crate clap;
extern crate signal_hook;

use clap::{Arg, App, ArgMatches};
use signal_hook::iterator::Signals;

mod request;
mod debugger;
mod notifier;

fn main() -> io::Result<()> {

    let args = get_config();

    let connection_string = get_connection_string(&args);
    let listener = TcpListener::bind(&connection_string)
                               .expect(&format!("Can't open TCP listener on {}", connection_string));

    println!("Listening on {}", connection_string);

    let notifier_rc = Arc::new(Mutex::new(notifier::Notifier::new()));

    let debug_cmd: Vec<String> = args.values_of("debug_cmd")
                                     .expect("Can't find program to debug, please rerun with correct parameters")
                                     .map(|x| x.to_string())
                                     .collect::<Vec<String>>();

    let debugger_rc = Arc::new(
        Mutex::new(
            debugger::get_debugger(args.value_of("debugger"), args.value_of("type"), Arc::clone(&notifier_rc))
        )
    );

    let thread_debugger = Arc::clone(&debugger_rc);

    let debugger_arg = match args.value_of("debugger") {
        Some(s) => s,
        None => "lldb",
    }.clone().to_string();

    let signals = Signals::new(&[signal_hook::SIGINT, signal_hook::SIGTERM])?;
    let signal_debugger = Arc::clone(&debugger_rc);
    thread::spawn(move || {
        for sig in signals.forever() {
            signal_debugger.lock()
                           .unwrap()
                           .debugger
                           .lock()
                           .unwrap()
                           .stop();
            println!("Terminated!");
            exit(0);
        }
    });

    thread::spawn(move || {
        thread_debugger.lock().unwrap().start(debugger_arg, &debug_cmd);
    });

    let mut handles = vec![];

    for stream in listener.incoming() {
        let stream = stream?;
        let notifier_stream = stream.try_clone().expect("Can't clone stream");

        // TODO: Something better than unwrap
        notifier_rc.lock().unwrap().add_listener(notifier_stream);

        let thread_notifier = Arc::clone(&notifier_rc);
        let thread_debugger = Arc::clone(&debugger_rc);

        let handle = thread::spawn(move || {
            request::handle_connection(stream, thread_notifier, thread_debugger);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn get_config<'a>() -> ArgMatches<'a> {
    let app = App::new("VIM Padre")
        .version("0.1.0")
        .author("Steven Trotter <stevetrot@gmail.com>")
        .about("A tool for building, debugging and reverse engineering in VIM")
        .long_about("Interfaces with 'lldb' or a similar debugger to debug programs and communicate with the vim-padre VIM plugin in order to effectively use VIM as a debugging interface.")
        .arg(Arg::with_name("port")
                 .short("p")
                 .long("port")
                 .takes_value(true)
                 .help("specify port to run on"))
        .arg(Arg::with_name("host")
                 .short("h")
                 .long("host")
                 .takes_value(true)
                 .help("specify host to run on"))
        .arg(Arg::with_name("debugger")
                 .short("d")
                 .long("debugger")
                 .takes_value(true)
                 .help("specify debugger to use"))
        .arg(Arg::with_name("type")
                 .short("t")
                 .long("type")
                 .takes_value(true)
                 .help("specify debugger type from [lldb, node, java, python]"))
        .arg(Arg::with_name("debug_cmd")
                 .multiple(true)
                 .takes_value(true))
        .get_matches();
    app
}

fn get_connection_string(args: &ArgMatches) -> String {
    let port = match args.value_of("port") {
        None => 12345,
        Some(s) => {
            match s.parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    panic!("Can't understand port");
                }
            }
        }
    };

    let host = match args.value_of("host") {
        None => "localhost",
        Some(s) => s
    };

    return format!("{}:{}", host, port)
}
