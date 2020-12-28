//! Handle a connection to the PADRE server including passing messages to and
//! from the debugger to the connection.

use std::io;
use std::net::SocketAddr;

use futures::prelude::*;
use tokio::net::{TcpListener, TcpStream};

use crate::debugger::Debugger;
use padre_core::vimcodec::VimFrame;


pub struct Server<'a> {
    connection_addr: SocketAddr,
    debugger: &'a Debugger,
}

impl<'a> Server<'a> {
    pub fn new(connection_addr: SocketAddr, debugger: &'a Debugger) -> Self {
        Server {
            connection_addr,
            debugger,
        }
    }

    /// Process a TCP listener.
    pub async fn process_connections(&self) {
        let listener = TcpListener::bind(&self.connection_addr)
            .map(|listener| {
                println!("Listening on {}", &self.connection_addr);
                listener
            })
            .await
            .expect(&format!(
                "Can't open TCP listener on {}",
                &self.connection_addr
            ));

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            self.process_connection(socket).await;
        }
    }

    /// Process a TCP socket connection.
    ///
    /// Fully sets up a new socket connection including listening for requests and sending responses.
    async fn process_connection(&self, socket: TcpStream) {
        let addr = socket.peer_addr().unwrap();

        let mut connection = Connection::new(socket);
    }
}

struct Connection {
    stream: TcpStream,
}

impl Connection {
    fn new(stream: TcpStream) -> Self {
        Connection { stream }
    }

    async fn read_frame(&mut self) -> Result<Option<VimFrame>, io::Error> {
        Ok(None)
    }
}
