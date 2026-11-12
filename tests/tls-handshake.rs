#![cfg(any(
    feature = "ssl-openssl",
    feature = "ssl-rustls",
    feature = "ssl-native-tls"
))]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn https_server() -> (tiny_http::Server, u16) {
    let server = tiny_http::Server::https(
        "127.0.0.1:0",
        tiny_http::SslConfig {
            certificate: include_bytes!("../examples/ssl-cert.pem").to_vec(),
            private_key: include_bytes!("../examples/ssl-key.pem").to_vec(),
        },
    )
    .unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    (server, port)
}

/// A peer that connects and never starts its TLS handshake must not keep the
/// server from serving other connections.
#[test]
fn silent_peer_does_not_block_other_connections() {
    let (_server, port) = https_server();

    // Connects, then stays silent for the whole test.
    let _silent = TcpStream::connect(("127.0.0.1", port)).unwrap();

    // A second peer is still attended to: its invalid handshake is rejected
    // and the server closes the connection, instead of leaving it unread.
    for _ in 0..3 {
        let mut other = TcpStream::connect(("127.0.0.1", port)).unwrap();
        other
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        other.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();

        let mut buf = Vec::new();
        // An alert followed by EOF, or a reset; a timeout means nobody is reading.
        if let Err(e) = other.read_to_end(&mut buf) {
            assert!(
                !matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ),
                "server did not attend to the connection: {}",
                e
            );
        }
    }
}
