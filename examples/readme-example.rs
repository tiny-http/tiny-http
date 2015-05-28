extern crate tiny_http;

fn main() {
    use tiny_http::{ServerBuilder, Response};

    let server = ServerBuilder::new().with_port(8000).build().unwrap();

    for request in server.incoming_requests() {
        println!("received request! method: {:?}, url: {:?}, headers: {:?}",
            request.get_method(),
            request.get_url(),
            request.get_headers()
        );

        let response = Response::from_string("hello world");
        request.respond(response);
    }
}
