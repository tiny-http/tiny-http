use std::io::net::tcp::TcpStream;
use std::time::duration::Duration;
use tiny_http;

/// Creates a server and a client connected to the server.
pub fn new_one_server_one_client() -> (tiny_http::Server, TcpStream) {
    let server = tiny_http::ServerBuilder::new().with_random_port().build().unwrap();
    let port = server.get_server_addr().port;
    let client = TcpStream::connect("127.0.0.1", port).unwrap();
    (server, client)
}

/// Creates a "hello world" server with a client connected to the server.
/// 
/// The server will automatically close after 3 seconds.
pub fn new_client_to_hello_world_server() -> TcpStream {
    let server = tiny_http::ServerBuilder::new().with_random_port().build().unwrap();
    let port = server.get_server_addr().port;
    let client = TcpStream::connect("127.0.0.1", port).unwrap();

    spawn(proc() {
        use std::io::timer;
        use time;

        let timeout = time::precise_time_ns()
            + 3 * 1000 * 1000 * 1000;

        loop {
            let rq = match server.try_recv().unwrap() {
                Some(rq) => {
                    let response = tiny_http::Response::from_string("hello world".to_string());
                    rq.respond(response);
                },
                _ => ()
            };

            timer::sleep(Duration::milliseconds(20));

            if timeout <= time::precise_time_ns() {
                break
            }
        }
    });

    client
}
