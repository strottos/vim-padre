//! Utilities
//!
//! Various simple utilities for use in PADRE

use std::net::TcpListener;

/// Get an unused port on the local system and return it. This port
/// can subsequently be used.
pub fn get_unused_localhost_port() -> u16 {
    let listener = TcpListener::bind(format!("127.0.0.1:0")).unwrap();
    listener.local_addr().unwrap().port()
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn find_and_use_unused_port() {
        let port = super::get_unused_localhost_port();
        thread::sleep(Duration::new(1, 0));
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }
}
