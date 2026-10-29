extern crate tiny_http;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

/// Connections that arrive together must all be read, even while every one of them is kept open:
/// each holds a pool thread until it closes, so none may be left queued without a thread.
#[test]
fn every_connection_of_a_burst_is_read_while_all_are_held() {
    const CLIENTS: usize = 64;
    let patience = Duration::from_secs(30);
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr().to_ip().unwrap();
    let at_once = Arc::new(Barrier::new(CLIENTS));
    let clients: Vec<_> = (0..CLIENTS)
        .map(|_| {
            let at_once = at_once.clone();
            thread::spawn(move || {
                at_once.wait();
                let mut c = TcpStream::connect_timeout(&addr, patience).unwrap();
                c.set_read_timeout(Some(patience)).unwrap();
                write!(c, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
                c
            })
        })
        .collect();
    let clients: Vec<TcpStream> = clients.into_iter().map(|c| c.join().unwrap()).collect();

    let mut held = Vec::new();
    while held.len() < CLIENTS {
        match server.recv_timeout(Duration::from_secs(5)) {
            Ok(Some(rq)) => held.push(rq),
            got => panic!(
                "{} of {} requests read, then {:?}",
                held.len(),
                CLIENTS,
                got
            ),
        }
    }
    for rq in held {
        rq.respond(tiny_http::Response::empty(200)).unwrap();
    }
    for (i, mut c) in clients.into_iter().enumerate() {
        c.shutdown(Shutdown::Write).unwrap();
        let mut answer = String::new();
        if let Err(e) = c.read_to_string(&mut answer) {
            panic!("client {}: {} after {:?}", i, e, answer);
        }
        assert!(answer.starts_with("HTTP/1.1 200"), "{}", answer);
    }
}
