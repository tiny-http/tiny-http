#![feature(test)]
extern crate test;

use std::io::Write;
use tiny_http::Method;

#[bench]
fn sequential_requests(bencher: &mut test::Bencher) {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    bencher.iter(|| {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        (write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
        let request = server.recv().unwrap();
        assert_eq!(request.method(), &Method::Get);
        assert_eq!(
            true,
            request
                .respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)))
                .is_ok()
        );
    });
}

#[bench]
fn parallel_requests(bencher: &mut test::Bencher) {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    bencher.iter(|| {
        for _ in 0..1000usize {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            (write!(
                stream,
                "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            ))
            .unwrap();
        }

        loop {
            let request = match server.try_recv().unwrap() {
                None => break,
                Some(rq) => rq,
            };
            assert_eq!(request.method(), &Method::Get);
            assert_eq!(
                true,
                request
                    .respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)))
                    .is_ok()
            );
        }
    });
}
