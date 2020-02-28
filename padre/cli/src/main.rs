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
//! The debug command should be specified as an addendum when running the command, e.g.
//! ```
//! padre -t=lldb -d=lldb -- my_program arg1 arg2 3 4
//! ```
//! will run the program `my_program arg1 arg2 3 4` in an `lldb` session.

use std::io;
use std::net::SocketAddr;
use std::process::exit;
use std::time::{Duration, Instant};

use clap::{App, Arg, ArgMatches};
use futures::prelude::*;
use tokio::net::TcpListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc::{self, Sender};
use tokio::time::delay_until;

use padre_core::debugger::{DebuggerCmd, DebuggerCmdBasic};
use padre_core::server;
use padre_core::util;

mod debugger;
use debugger::create_debugger;

fn get_app_args<'a>() -> ArgMatches<'a> {
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

fn exit_padre(mut debugger_queue_tx: Sender<(DebuggerCmd, Instant)>) {
    tokio::spawn(async move {
        let instant = Instant::now() + Duration::new(5,0);
        let command = (DebuggerCmd::Basic(DebuggerCmdBasic::Exit), instant);
        debugger_queue_tx.send(command).await.unwrap();
        delay_until(tokio::time::Instant::from_std(instant)).await;
        println!("Timed out exiting!");
        exit(-1);
    });
}

async fn run_padre() -> io::Result<()> {
    let args = get_app_args();

    let debug_cmd: Vec<String> = args
        .values_of("debug_cmd")
        .expect("Can't find program to debug, please rerun with correct parameters")
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    let (debugger_queue_tx, debugger_queue_rx) = mpsc::channel(128);

    // TODO: Do we need to wrap in Arc/Mutex any more now/when we're on new tokio 0.2? Probably in
    // the case of multiple connections but is there a way around it?
    let _debugger = create_debugger(
        args.value_of("debugger"),
        args.value_of("type"),
        debug_cmd,
        debugger_queue_rx,
    );

    let connection_addr = get_connection(&args);
    let mut socket = TcpListener::bind(&connection_addr)
        .map(|listener| {
            println!("Listening on {}", &connection_addr);
            listener
        })
        .await
        .expect(&format!("Can't open TCP listener on {}", &connection_addr));

    let mut incoming = socket.incoming();

    // TODO: Merge the following into one lot of signals when we know how to

    let debugger_signals_queue_tx = debugger_queue_tx.clone();
    tokio::spawn(async move {
        let mut signals = signal(SignalKind::interrupt()).unwrap();
        let debugger_signals_queue_tx = debugger_signals_queue_tx.clone();

        while let Some(_) = signals.recv().await {
            exit_padre(debugger_signals_queue_tx.clone());
        }
    });

    let debugger_signals_queue_tx = debugger_queue_tx.clone();
    tokio::spawn(async move {
        let mut signals = signal(SignalKind::quit()).unwrap();
        let debugger_signals_queue_tx = debugger_signals_queue_tx.clone();

        while let Some(_) = signals.recv().await {
            exit_padre(debugger_signals_queue_tx.clone());
        }
    });

    let debugger_signals_queue_tx = debugger_queue_tx.clone();
    tokio::spawn(async move {
        let mut signals = signal(SignalKind::terminate()).unwrap();
        let debugger_signals_queue_tx = debugger_signals_queue_tx.clone();

        while let Some(_) = signals.recv().await {
            exit_padre(debugger_signals_queue_tx.clone());
        }
    });

    while let Some(Ok(stream)) = incoming.next().await {
        let debugger_queue_tx = debugger_queue_tx.clone();
        tokio::spawn(async move {
            server::process_connection(stream, debugger_queue_tx.clone());
        });
    }

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let padre = tokio::spawn(run_padre());
    padre.await.unwrap()
}
