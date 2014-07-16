#![feature(phase)]

extern crate httpd = "tiny-http";
extern crate test;

use std::io::Command;

#[test]
#[ignore]
// TODO: obtain time
fn curl_bench() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let num_requests = 10u;

    match Command::new("curl")
        .arg("-s")
        .arg(format!("http://localhost:{}/?[1-{}]", port, num_requests).as_slice())
        .output()
    {
        Ok(p) => p,
        Err(_) => return,       // ignoring test
    };

    drop(server);
}

#[bench]
fn sequential_requests(bencher: &mut test::Bencher) {
    ::std::io::test::raise_fd_limit();

    let (server, port) = httpd::Server::new_with_random_port().unwrap();

    let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();

    bencher.auto_bench(|_| {
        (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();

        let request = server.recv().unwrap();

        assert!(request.get_method().equiv(&"get"));

        request.respond(httpd::Response::new_empty(httpd::StatusCode(204)));
    });
}

#[bench]
fn parallel_requests(bencher: &mut test::Bencher) {
    ::std::io::test::raise_fd_limit();

    let (server, port) = httpd::Server::new_with_random_port().unwrap();

    bencher.bench_n(5, |_| {
        let mut streams = Vec::new();

        for _ in range(0u, 1000) {
            let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();
            (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")).unwrap();
            streams.push(stream);
        }

        loop {
            let request = match server.try_recv().unwrap() {
                None => break,
                Some(rq) => rq
            };

            assert!(request.get_method().equiv(&"get"));

            request.respond(httpd::Response::new_empty(httpd::StatusCode(204)));
        }
    });
}
