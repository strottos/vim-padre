extern crate clap;

use clap::{Arg, App, ArgMatches};
use std::io;
use std::net::{TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

mod request;
mod debugger;
mod notifier;

fn main() -> io::Result<()> {
    let matches = get_config();

    let connection_string = get_connection_string(&matches);
    let listener = TcpListener::bind(&connection_string)
                               .expect(&format!("Can't open TCP listener on {}", connection_string));

    println!("Listening on {}", connection_string);

    let the_notifier = Arc::new(Mutex::new(notifier::Notifier::new()));

    let debug_cmd: Vec<_> = matches.values_of("debug_cmd")
                                   .expect("Can't find program to debug, please rerun with correct parameters")
                                   .collect();

    let debugger_rc = Arc::new(
        Mutex::new(
            debugger::get_debugger(&debug_cmd, matches.value_of("debugger"), Arc::clone(&the_notifier))
        )
    );

    let mut handles = vec![];

    for stream in listener.incoming() {
        let stream = stream?;
        let notifier_stream = stream.try_clone().expect("Can't clone stream");

        // TODO: Something better than unwrap
        the_notifier.lock().unwrap().add_listener(notifier_stream);

        let notifier_clone = Arc::clone(&the_notifier);
        let debugger_connection = Arc::clone(&debugger_rc);

        let handle = thread::spawn(move || {
            request::handle_connection(stream, notifier_clone, debugger_connection);
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
                 .help("specify debugger to use from [lldb, node, java, python]"))
        .arg(Arg::with_name("debug_cmd")
                 .multiple(true)
                 .takes_value(true))
        .get_matches();
    app
}

fn get_connection_string(matches: &ArgMatches) -> String {
    let port = match matches.value_of("port") {
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

    let host = match matches.value_of("host") {
        None => "localhost",
        Some(s) => s
    };

    return format!("{}:{}", host, port)
}
