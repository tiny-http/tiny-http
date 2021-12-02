extern crate tiny_http;

use std::io::{Read, Write};

#[allow(dead_code)]
mod support;

#[cfg(test)]
fn basic_handling_impl(split_join: bool) {
    let (server, mut stream) = support::new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();
    assert!(*request.method() == tiny_http::Method::Get);
    //assert!(request.url() == "/");
    let response = tiny_http::Response::from_string("hello world".to_owned());
    let response = if split_join {
        let (reader, params) = response.split();
        tiny_http::Response::join(reader, params)
    } else {
        response
    };
    request.respond(response).unwrap();

    server.try_recv().unwrap();

    let mut content = String::new();
    stream.read_to_string(&mut content).unwrap();
    assert!(content.ends_with("hello world"));
}

#[test]
fn basic_handling() {
    basic_handling_impl(false);
    basic_handling_impl(true);
}
