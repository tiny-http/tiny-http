extern crate httpd = "tiny-http";

use std::io::net::tcp::TcpStream;

#[test]
#[ignore]
fn basic_handling() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();

    let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();
    write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");

    let request = server.recv().unwrap();
    assert!(request.get_method().equiv(&"get"));
    //assert!(request.get_url() == "/");
    request.respond(httpd::Response::from_string(format!("hello world")));

    server.try_recv().unwrap();

    let content = stream.read_to_string().unwrap();
    assert!(content.as_slice().ends_with("hello world"));
}
