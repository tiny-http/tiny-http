extern crate tiny_http;

use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};

#[allow(dead_code)]
mod support;
use chunked_transfer::Decoder;
use support::{new_one_server_one_client, new_one_server_one_client_unbuffered, SlowByteSrc};

#[test]
fn basic_handling() {
    let (server, mut stream) = new_one_server_one_client();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();
    assert!(*request.method() == tiny_http::Method::Get);
    assert!(request.url() == "/");
    request
        .respond(tiny_http::Response::from_string("hello world".to_owned()))
        .unwrap();

    server.try_recv().unwrap();

    let mut content = String::new();
    stream.read_to_string(&mut content).unwrap();
    assert!(content.ends_with("hello world"));
}

#[test]
fn unbuffered() {
    let (server, mut stream) = new_one_server_one_client_unbuffered();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let request = server.recv().unwrap();
    let start = Instant::now();
    std::thread::spawn(|| {
        request
            .respond(
                tiny_http::Response::new(
                    tiny_http::StatusCode(200),
                    Vec::new(),
                    SlowByteSrc {
                        sleep_time: Duration::from_millis(50),
                        val: 65,
                        // extreme length (100GB): we ensure that the data
                        // is streamed on demand instead of assembled in memory
                        len: 100_000_000_000,
                    },
                    None,
                    None,
                )
                .with_buffering(tiny_http::BufferingMode::Unbuffered),
            )
            .unwrap()
    });

    assert!(server.try_recv().unwrap().is_none());

    let mut buf = [0; 64 * 1024];
    let mut read = 0;

    loop {
        let nb_read = stream.read(&mut buf[read..]).unwrap();

        // we should receive some data, but only a small amount because of the lack
        // of buffering
        assert!(nb_read > 0);
        assert!(nb_read < buf.len());

        read += nb_read;

        // ensure that we receive the data quickly after the SlowByteSrc reader
        // started feeeding the server
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(75) {
            break;
        }
    }

    let res = String::from_utf8(buf[..read].to_vec()).expect("Invalid UTF8 characters");
    assert!(res.contains("Transfer-Encoding: chunked"));

    let chunked_index = res
        .find("\r\n\r\n")
        .expect("Could not find the start of the chunked messages");
    let mut chunked_data = res[chunked_index + 4..].to_string();
    // emit the "end of stream" message
    chunked_data.push_str("0\r\n\r\n");

    // verify that we only received '\x65' characters
    let mut decoder = Decoder::new(chunked_data.as_bytes());
    let mut decoded = String::new();
    decoder
        .read_to_string(&mut decoded)
        .expect("Invalid (non-chunked?) data");
    let bytes = decoded.as_bytes();
    let mut expected_vec = Vec::with_capacity(bytes.len());
    for _ in 0..bytes.len() {
        expected_vec.push(65);
    }
    assert!(bytes == expected_vec.as_slice());
}
