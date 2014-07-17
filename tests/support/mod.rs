use httpd;
use std::io::net::tcp::TcpStream;

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client() -> (httpd::Server, TcpStream) {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let client = TcpStream::connect("127.0.0.1", port).unwrap();
    (server, client)
}
