extern crate tiny_http;

#[cfg(not(feature = "ssl"))]
fn main() { println!("This example requires the `ssl` feature to be enabled"); }

#[cfg(feature = "ssl")]
fn main() {
    use tiny_http::{Server, Response};

    let server = Server::https("0.0.0.0:8000", tiny_http::SslConfig {
        certificate: include_bytes!("ssl-cert.pem").to_vec(),
        private_key: include_bytes!("ssl-key.pem").to_vec(),
    }).unwrap();

    for request in server.incoming_requests() {
        println!("received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );

        let response = Response::from_string("hello world");
        request.respond(response);
    }
}
