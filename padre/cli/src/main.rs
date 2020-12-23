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
//!            - godlv (Go Delve Debugger)
//!   -d/--debugger
//!
//! The debug command should be specified as an addendum when running the command, e.g.
//! ```
//! padre -t=lldb -d=lldb -- my_program arg1 arg2 3 4
//! ```
//! will run the program `my_program arg1 arg2 3 4` in an `lldb` session.

use std::io;
use std::net::SocketAddr;

use clap::{App, Arg, ArgMatches};
use futures::prelude::*;
use tokio::net::TcpListener;
use tokio::runtime;

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
                 .help("specify debugger type from [lldb, node, python, godlv]"))
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

fn main() -> io::Result<()> {
    let rt = runtime::Builder::new_current_thread()
        .build()?;

    rt.block_on(async {
        let args = get_app_args();

        let debug_cmd: Vec<String> = args
            .values_of("debug_cmd")
            .expect("Can't find program to debug, please rerun with correct parameters")
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        let debugger = create_debugger(args.value_of("debugger"), args.value_of("type"), debug_cmd);

        let connection_addr = get_connection(&args);
        let mut listener = TcpListener::bind(&connection_addr)
            .map(|listener| {
                println!("Listening on {}", &connection_addr);
                listener
            })
            .await
            .expect(&format!("Can't open TCP listener on {}", &connection_addr));

        server::process_connection(listener, debugger);

        Ok(())
    })
}
