extern crate tiny_http;
extern crate time;

#[allow(dead_code)]
mod support;

#[test]
fn basic_string_input() {
    let (server, client) = support::new_one_server_one_client();

    {
        let mut client = client;
        (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 5\r\n\r\nhello")).unwrap();
    }

    let mut request = server.recv().unwrap();

    assert_eq!(request.as_reader().read_to_string().unwrap().as_slice(), "hello");
}

#[test]
fn wrong_content_length() {
    let (server, client) = support::new_one_server_one_client();

    {
        let mut client = client;
        (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 3\r\n\r\nhello")).unwrap();
    }

    let mut request = server.recv().unwrap();

    assert_eq!(request.as_reader().read_to_string().unwrap().as_slice(), "hel");
}

#[test]
fn expect_100_continue() {
    let (server, client) = support::new_one_server_one_client();

    let mut client = client;
    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nExpect: 100-continue\r\nContent-Type: text/plain; charset=utf8\r\nContent-Length: 5\r\n\r\n")).unwrap();
    client.flush().unwrap();

    let (tx, rx) = channel();

    spawn(proc() {
        let mut request = server.recv().unwrap();
        assert_eq!(request.as_reader().read_to_string().unwrap().as_slice(), "hello");
        tx.send(());
    });

    client.set_timeout(Some(300));
    let content = client.read_exact(12).unwrap();
    assert!(content.as_slice().slice_from(9).starts_with(b"100"));   // 100 status code

    (write!(client, "hello")).unwrap();
    client.flush().unwrap();
    client.close_write().unwrap();

    rx.recv();
}

#[test]
fn unsupported_expect_header() {
    let mut client = support::new_client_to_hello_world_server();

    (write!(client, "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nExpect: 189-dummy\r\nContent-Type: text/plain; charset=utf8\r\n\r\n")).unwrap();

    client.set_timeout(Some(300));
    let content = client.read_to_string().unwrap();
    assert!(content.as_slice().slice_from(9).starts_with("417"));   // 417 status code
}
