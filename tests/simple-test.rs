extern crate tiny_http;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[allow(dead_code)]
mod support;

#[test]
fn basic_handling() {
    let (server, mut stream) = support::new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();
    assert!(*request.method() == tiny_http::Method::Get);
    //assert!(request.url() == "/");
    request
        .respond(tiny_http::Response::from_string("hello world".to_owned()))
        .unwrap();

    server.try_recv().unwrap();

    let mut content = String::new();
    stream.read_to_string(&mut content).unwrap();
    assert!(content.ends_with("hello world"));
}

#[test]
fn parse_v4_literal() {
    let addr: tiny_http::ConfigListenAddr = "127.0.0.1:8080".parse().unwrap();

    match addr {
        tiny_http::ConfigListenAddr::IP(v) => {
            assert_eq!(1, v.len());
            assert_eq!(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                *v.first().unwrap()
            );
        }
        _ => panic!("Not an IP listener"),
    }
}

#[test]
fn parse_v6_literal() {
    let addr: tiny_http::ConfigListenAddr = "[2001:db8::1]:8080".parse().unwrap();

    match addr {
        tiny_http::ConfigListenAddr::IP(v) => {
            assert_eq!(1, v.len());
            assert_eq!(
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    8080
                ),
                *v.first().unwrap()
            );
        }
        _ => panic!("Not an IP listener"),
    }
}
