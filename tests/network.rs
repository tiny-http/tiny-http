extern crate httpd = "tiny-http";
extern crate time;

use std::io::timer;

#[allow(dead_code)]
mod support;

#[test]
fn connection_close_header() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\n")).unwrap();
    timer::sleep(1000);

    (write!(client, "GET / HTTP/1.1\r\nConnection: close\r\n\r\n")).unwrap();

    // if the connection was not closed, this will err with timeout
    client.set_timeout(Some(100));
    client.read_to_end().unwrap();
}

#[test]
fn http_1_0_connection_close() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")).unwrap();

    // if the connection was not closed, this will err with timeout
    client.set_timeout(Some(100));
    client.read_to_end().unwrap();
}

#[test]
fn detect_connection_closed() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\n")).unwrap();
    timer::sleep(1000);

    client.close_write();

    // if the connection was not closed, this will err with timeout
    client.set_timeout(Some(100));
    client.read_to_end().unwrap();
}

#[test]
fn poor_network_test() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "G")).unwrap();
    timer::sleep(100);
    (write!(client, "ET /he")).unwrap();
    timer::sleep(100);
    (write!(client, "llo HT")).unwrap();
    timer::sleep(100);
    (write!(client, "TP/1.")).unwrap();
    timer::sleep(100);
    (write!(client, "1\r\nHo")).unwrap();
    timer::sleep(100);
    (write!(client, "st: localho")).unwrap();
    timer::sleep(100);
    (write!(client, "st\r\nConnec")).unwrap();
    timer::sleep(100);
    (write!(client, "tion: close\r")).unwrap();
    timer::sleep(100);
    (write!(client, "\n\r")).unwrap();
    timer::sleep(100);
    (write!(client, "\n")).unwrap();

    client.set_timeout(Some(200));
    let data = client.read_to_string().unwrap();
    assert!(data.as_slice().ends_with("hello world"));
}

#[test]
fn pipelining_test() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(client, "GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(client, "GET /world HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")).unwrap();

    client.set_timeout(Some(200));
    let data = client.read_to_string().unwrap();
    assert_eq!(data.as_slice().split_str("hello world").count(), 4);
}

#[test]
fn server_crash_results_in_response() {
    use std::io::net::tcp::TcpStream;

    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let mut client = TcpStream::connect("127.0.0.1", port).unwrap();

    spawn(proc() {
        server.recv().unwrap();
        // oops, server crash
    });

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")).unwrap();

    client.set_timeout(Some(200));
    let content = client.read_to_string().unwrap();
    assert!(content.as_slice().slice_from(9).starts_with("5"));   // 5xx status code
}

#[test]
fn responses_reordered() {
    use std::io::timer;

    let (server, mut client) = support::new_one_server_one_client();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).unwrap();
    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")).unwrap();

    spawn(proc() {
        let rq1 = server.recv().unwrap();
        let rq2 = server.recv().unwrap();

        spawn(proc() {  
            rq2.respond(httpd::Response::from_string(format!("second request")));
        });

        timer::sleep(100);

        spawn(proc() {  
            rq1.respond(httpd::Response::from_string(format!("first request")));
        });
    });

    client.set_timeout(Some(200));
    let content = client.read_to_string().unwrap();
    assert!(content.as_slice().ends_with("second request"));
}

#[test]
fn connection_timeout() {
    let (server, mut client) = support::new_one_server_one_client();
    let (tx_stop, rx_stop) = channel();

    // executing server in parallel
    spawn(proc() {
        loop {
            server.try_recv();
            timer::sleep(100);
            if rx_stop.try_recv().is_ok() { break }
        }
    });

    // waiting for the 408 response
    let content = client.read_to_string().unwrap();
    assert!(content.as_slice().slice_from(9).starts_with("408"));

    // stopping server
    tx_stop.send(());
}
