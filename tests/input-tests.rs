extern crate httpd = "tiny-http";

use std::io::net::tcp::TcpStream;

#[test]
#[ignore]   // failing
fn input_basic_string() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();

    {
        let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();
        write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 5\r\n\r\nhello");
    }

    let mut request = server.recv().unwrap();

    assert_eq!(request.as_reader().read_to_string().unwrap().as_slice(), "hello");
}
