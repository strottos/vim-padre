#[macro_use]
extern crate lazy_static;
extern crate clap;
extern crate regex;
extern crate signal_hook;
#[macro_use]
extern crate futures;
extern crate bytes;
extern crate tokio;

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use clap::{App, Arg, ArgMatches};
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
//use signal_hook::iterator::Signals;

mod debugger;
mod notifier;
mod request;

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

fn get_connection(args: &ArgMatches) -> SocketAddr {
    let port = match args.value_of("port") {
        None => 12345,
        Some(s) => match s.parse::<i32>() {
            Ok(n) => n,
            Err(_) => {
                panic!("Can't understand port");
            }
        },
    };

    let host = match args.value_of("host") {
        None => "0.0.0.0",
        Some(s) => s,
    };

    return format!("{}:{}", host, port).parse::<SocketAddr>().unwrap();
}

//fn install_signals(signals: Signals, debugger: Arc<Mutex<debugger::PadreServer>>) {
//    thread::spawn(move || {
//        for _ in signals.forever() {
//            match debugger.lock() {
//                Ok(s) => {
//                    match s.debugger.lock() {
//                        Ok(t) => t.stop(),
//                        Err(err) => println!("Debugger not found: {}", err),
//                    };
//                },
//                Err(err) => println!("Debug server not found: {}", err),
//            };
//            println!("Terminated!");
//            exit(0);
//        }
//    });
//}

fn main() -> io::Result<()> {
    let args = get_config();

    let connection_string = get_connection(&args);
    let listener = TcpListener::bind(&connection_string)
        .expect(&format!("Can't open TCP listener on {}", connection_string));

    println!("Listening on {}", connection_string);

    let notifier_rc = Arc::new(Mutex::new(notifier::Notifier::new()));

    let debug_cmd: Vec<String> = args
        .values_of("debug_cmd")
        .expect("Can't find program to debug, please rerun with correct parameters")
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    let (padre_server, padre_process) = debugger::get_debugger(
        args.value_of("debugger"),
        args.value_of("type"),
        debug_cmd,
        Arc::clone(&notifier_rc),
    );

    let padre_server_rc = Arc::new(Mutex::new(padre_server));

    //    let signals = Signals::new(&[signal_hook::SIGINT, signal_hook::SIGTERM])?;
    //    install_signals(signals, Arc::clone(&padre_server_rc));

    let mut runtime = Runtime::new().unwrap();

    let request_debugger = Arc::clone(&padre_server_rc);
    let request_notifier = Arc::clone(&notifier_rc);

    runtime.spawn(
        listener
            .incoming()
            .map_err(|e| eprintln!("failed to accept socket; error = {:?}", e))
            .for_each(move |socket| {
                let padre_connection = request::PadreConnection::new(
                    socket,
                    Arc::clone(&request_notifier),
                    Arc::clone(&request_debugger),
                );

                tokio::spawn(
                    padre_connection
                        .for_each(|a| {
                            println!("Main foreach a: {:?}", a);
                            Ok(())
                        })
                        .map_err(|e| {
                            println!("connection error = {:?}", e);
                        }),
                );

                Ok(())
            }),
    );

    // TODO: Spawn debugger process as future
    runtime.spawn(padre_process.map_err(|e| {
        println!("connection error = {:?}", e);
    }));

    runtime.run().unwrap();

    Ok(())
}
