//! Notifier
//!
//! This module contains tools for notifying every socket connection about an
//! event.

use std::net::SocketAddr;

/// A `Listener` is a wrapper around the ...
#[derive(Debug)]
struct Listener {
    addr: SocketAddr,
    has_started: bool,
}

/// The `Notifier` creates the main singleton object for PADRE to communicate
/// with it's listeners.
///
/// We store a vector of `Listener`s and when one is finished with we drop it
/// from the list.
#[derive(Debug)]
pub struct Notifier {
    listeners: Vec<Listener>,
}

