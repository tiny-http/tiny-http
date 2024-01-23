extern crate tiny_http;

#[cfg(not(feature = "ssl-rustls"))]
fn main() {
    println!("This example requires one of the supported `ssl-*` features to be enabled");
}

#[cfg(feature = "ssl-rustls")]
fn main() {
    use tiny_http::{ssl, Response, Server};
    let server_config = ssl::SslContextImpl::get_server_config_from_pem(
        include_bytes!("ssl-cert.pem").to_vec(),
        include_bytes!("ssl-key.pem").to_vec().into(),
    );
    let server = Server::https("0.0.0.0:8000", None, server_config.unwrap()).unwrap();
    println!(
        "Note: connecting to this server will likely give you a warning from your browser \
              because the connection is unsecure. This is because the certificate used by this \
              example is self-signed. With a real certificate, you wouldn't get this warning."
    );

    for request in server.incoming_requests() {
        assert!(request.secure());
        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );

        let response = Response::from_string("hello world");
        request
            .respond(response)
            .unwrap_or(println!("Failed to respond to request"));
    }
}
