#![feature(phase)]

extern crate tiny_http;
extern crate test;
extern crate time;

use std::process::Command;

#[test]
#[ignore]
// TODO: obtain time
fn curl_bench() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().port;
    let num_requests = 10usize;

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

    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().port;

    let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();

    bencher.auto_bench(|_| {
        (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();

        let request = server.recv().unwrap();

        assert!(request.method().equiv(&"get"));

        request.respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)));
    });
}

#[bench]
fn parallel_requests(bencher: &mut test::Bencher) {
    ::std::io::test::raise_fd_limit();

    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().port;

    bencher.bench_n(5, |_| {
        let mut streams = Vec::new();

        for _ in 0..1000usize {
            let mut stream = std::io::net::tcp::TcpStream::connect("127.0.0.1", port).unwrap();
            (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")).unwrap();
            streams.push(stream);
        }

        loop {
            let request = match server.try_recv().unwrap() {
                None => break,
                Some(rq) => rq
            };

            assert!(request.method().equiv(&"get"));

            request.respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)));
        }
    });
}
