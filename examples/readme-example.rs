extern crate tiny_http;

fn main() {
    use tiny_http::{ServerBuilder, Response};

    let server = ServerBuilder::new().with_port(8000).build().unwrap();

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
