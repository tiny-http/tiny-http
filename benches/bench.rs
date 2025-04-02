extern crate fdlimit;
extern crate tiny_http;

use divan::counter::ItemsCount;
use divan::{bench, Bencher, Divan};
use std::io::Write;
use std::net;
use std::time::Duration;
use tiny_http::Method;

fn main() {
    // Run registered benchmarks
    Divan::default()
        .min_time(Duration::from_secs(2))
        .config_with_args()
        .main();
}

#[test]
#[ignore]
// TODO: obtain time
fn curl_bench() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let num_requests = 10usize;

    match std::process::Command::new("curl")
        .arg("-s")
        .arg(format!("http://localhost:{}/?[1-{}]", port, num_requests))
        .output()
    {
        Ok(p) => p,
        Err(_) => return, // ignoring test
    };

    drop(server);
}

#[bench]
fn sequential_requests(bencher: Bencher) {
    let server = tiny_http::Server::http((net::Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    let mut stream = net::TcpStream::connect((net::Ipv4Addr::LOCALHOST, port)).unwrap();

    bencher.bench_local(|| {
        write!(stream, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();

        let request = server.recv().unwrap();

        assert_eq!(request.method(), &Method::Get);

        request
            .respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)))
            .unwrap();
    });
}

#[bench(args = [10, 100, 1000])]
fn parallel_requests(bencher: Bencher, num_requests: u64) {
    fdlimit::raise_fd_limit();

    let server = tiny_http::Server::http((net::Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    bencher.counter(ItemsCount::new(num_requests)).bench(|| {
        let mut streams = Vec::new();

        for _ in 0..num_requests {
            let mut stream = net::TcpStream::connect((net::Ipv4Addr::LOCALHOST, port)).unwrap();
            write!(
                stream,
                "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            streams.push(stream);
        }

        while let Some(request) = server.try_recv().unwrap() {
            assert_eq!(request.method(), &Method::Get);

            request
                .respond(tiny_http::Response::new_empty(tiny_http::StatusCode(204)))
                .unwrap();
        }
    });
}
