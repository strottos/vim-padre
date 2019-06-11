#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

use std::io;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

use clap::{App, Arg, ArgMatches};
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use signal_hook::iterator::Signals;

mod debugger;
mod notifier;
mod request;
mod server;

fn get_config<'a>() -> ArgMatches<'a> {
    let app = App::new("VIM Padre")
        .version("0.1.0")
        .author("Steven Trotter <stevetrot@gmail.com>")
        .about("A tool for building, debugging and reverse engineering in VIM")
        .long_about("Interfaces with 'lldb' or a similar debugger to debug programs and communicate with the Vim PADRE plugin in order to effectively use Vim as a debugging interface.")
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

fn install_signals(signals: Signals, debugger: Arc<Mutex<debugger::PadreDebugger>>) {
    thread::spawn(move || {
        for _ in signals.forever() {
            match debugger.lock() {
                Ok(mut s) => {
                    s.stop();
                },
                Err(e) => println!("Debug server not found: {}", e),
            };
            println!("Terminated!");
            exit(0);
        }
    });
}

struct Runner {}

impl Future for Runner {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let args = get_config();

        let connection_string = get_connection(&args);
        let listener = TcpListener::bind(&connection_string)
            .expect(&format!("Can't open TCP listener on {}", connection_string));

        println!("Listening on {}", connection_string);

        let notifier = Arc::new(Mutex::new(notifier::Notifier::new()));

        let debug_cmd: Vec<String> = args
            .values_of("debug_cmd")
            .expect("Can't find program to debug, please rerun with correct parameters")
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        let debugger = Arc::new(Mutex::new(debugger::get_debugger(
            args.value_of("debugger"),
            args.value_of("type"),
            debug_cmd,
            notifier.clone(),
        )));

        let signals = Signals::new(&[signal_hook::SIGINT, signal_hook::SIGTERM]).unwrap();
        install_signals(signals, debugger.clone());

        tokio::spawn(
            listener
                .incoming()
                .map_err(|e| eprintln!("failed to accept socket; error = {:?}", e))
                .for_each(move |socket| {
                    let debugger = debugger.clone();
                    let notifier = notifier.clone();
                    server::process_connection(socket, debugger, notifier);

                    Ok(())
                }),
        );

        Ok(Async::Ready(()))
    }
}

fn main() -> io::Result<()> {
    let mut runtime = Runtime::new().unwrap();

    runtime.spawn(Runner {});

    runtime.run().unwrap();

    Ok(())
}
