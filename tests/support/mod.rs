use std::thread::{self, sleep};
use std::time::Duration;
use std::{io::Read, net::TcpStream};

use tiny_http::{ConfigListenAddr, ServerConfig, ServerConfigAdvanced};

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client_custom_config(
    config: ServerConfig,
) -> (tiny_http::Server, TcpStream) {
    let server = tiny_http::Server::new(config).unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    (server, client)
}

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client() -> (tiny_http::Server, TcpStream) {
    new_one_server_one_client_custom_config(ServerConfig {
        addr: ConfigListenAddr::from_socket_addrs("0.0.0.0:0").unwrap(),
        ssl: None,
        advanced: ServerConfigAdvanced::new(),
    })
}

/// Creates a server and a client connected to the server, with an unbuffered writer.
pub fn new_one_server_one_client_unbuffered() -> (tiny_http::Server, TcpStream) {
    new_one_server_one_client_custom_config(ServerConfig {
        addr: ConfigListenAddr::from_socket_addrs("0.0.0.0:0").unwrap(),
        ssl: None,
        advanced: ServerConfigAdvanced::new()
            .with_writer_buffering_mode(tiny_http::BufferingMode::Unbuffered),
    })
}

/// Creates a "hello world" server with a client connected to the server.
///
/// The server will automatically close after 3 seconds.
pub fn new_client_to_hello_world_server() -> TcpStream {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let client = TcpStream::connect(("127.0.0.1", port)).unwrap();

    thread::spawn(move || {
        let mut cycles = 3 * 1000 / 20;

        loop {
            if let Some(rq) = server.try_recv().unwrap() {
                let response = tiny_http::Response::from_string("hello world".to_string());
                rq.respond(response).unwrap();
            }

            thread::sleep(Duration::from_millis(20));

            cycles -= 1;
            if cycles == 0 {
                break;
            }
        }
    });

    client
}

/// Stream that produces bytes very slowly
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SlowByteSrc {
    pub sleep_time: Duration,
    pub val: u8,
    pub len: usize,
}

impl<'b> Read for SlowByteSrc {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        sleep(self.sleep_time);
        let l = self.len.min(buf.len()).min(1000);
        for v in buf[..l].iter_mut() {
            *v = self.val;
        }
        self.len -= l;
        Ok(l)
    }
}
