use httpd;
use std::io::net::tcp::TcpStream;

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client() -> (httpd::Server, TcpStream) {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let client = TcpStream::connect("127.0.0.1", port).unwrap();
    (server, client)
}

/// Creates a "hello world" server with a client connected to the server.
///
/// You must specify the number of requests that the server will receive before closing.
pub fn new_client_to_hello_world_server(num: uint) -> TcpStream {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let client = TcpStream::connect("127.0.0.1", port).unwrap();

    spawn(proc() {
        for _ in range(0, num) {
            let rq = server.recv().unwrap();
            let response = httpd::Response::from_string("hello world".to_string());
            rq.respond(response);
        }
    });

    client
}
