//! PADRE Debugger
//!
//! This program creates a socket interface that enables debuggers to communicate
//! in a standard manner with multiple different debuggers and programming languages.
//! Options supported:
//!   -p/--port   Port to run socket interface on
//!   -h/--host   Hostname to run on
//!   -t/--type   The type of debugger to spawn
//!          Currently supported are
//!            - lldb
//!            - node
//!            - python
//!   -d/--debugger
//!
//! The debug command should be specified as an addendum when running the command

#[macro_use]
extern crate lazy_static;

use std::io;
use std::net::SocketAddr;
use std::process::exit;
use std::time::{Duration, Instant};

use clap::{App, Arg, ArgMatches};
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;
use tokio_signal::unix::{Signal, SIGINT, SIGQUIT, SIGTERM};

mod config;
mod notifier;
mod util;

fn get_config<'a>() -> ArgMatches<'a> {
    App::new("VIM Padre")
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
        .get_matches()
}

fn get_connection(args: &ArgMatches) -> SocketAddr {
    let port = match args.value_of("port") {
        None => util::get_unused_localhost_port(),
        Some(s) => match s.parse::<u16>() {
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

fn exit_padre() {
    let when = Instant::now() + Duration::new(5, 0);

    tokio::spawn({
        Delay::new(when)
            .map_err(|e| panic!("timer failed; err={:?}", e))
            .and_then(|_| {
                println!("Timed out exiting!");
                exit(-1);
                #[allow(unreachable_code)]
                Ok(())
            })
    });
}

struct Runner {}

impl Future for Runner {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let args = get_config();

        let connection_addr = get_connection(&args);
        let listener = TcpListener::bind(&connection_addr)
            .map(|listener| {
                println!("Listening on {}", &connection_addr);
                listener
            })
            .expect(&format!("Can't open TCP listener on {}", &connection_addr));

        let signals = Signal::new(SIGINT)
            .flatten_stream()
            .for_each(move |_| {
                exit_padre();
                Ok(())
            })
            .map_err(|e| {
                println!("Caught SIGINT Error: {:?}", e);
            });

        let signals = Signal::new(SIGQUIT)
            .flatten_stream()
            .for_each(move |_| {
                exit_padre();
                Ok(())
            })
            .map_err(|e| {
                println!("Caught SIGQUIT Error: {:?}", e);
            })
            .join(signals)
            .map(|_| {});

        let signals = Signal::new(SIGTERM)
            .flatten_stream()
            .for_each(move |_| {
                exit_padre();
                Ok(())
            })
            .map_err(|e| {
                println!("Caught SIGTERM Error: {:?}", e);
            })
            .join(signals)
            .map(|_| {});

        tokio::spawn(signals);

        tokio::spawn(
            listener
                .incoming()
                .map_err(|e| eprintln!("failed to accept socket; error = {:?}", e))
                .for_each(move |socket| {
                    println!("socket {:?}", socket);
                    Ok(())
                })
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
