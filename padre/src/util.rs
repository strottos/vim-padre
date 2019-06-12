use std::net::TcpListener;

pub fn get_unused_localhost_port() -> u16 {
    let listener = TcpListener::bind(format!("127.0.0.1:0")).unwrap();
    listener.local_addr().unwrap().port()
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    #[test]
    fn find_and_use_unused_port() {
        let port = super::get_unused_localhost_port();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }
}
