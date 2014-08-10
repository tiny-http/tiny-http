extern crate tiny_http;

fn main() {
    let server = tiny_http::ServerBuilder::new().build().unwrap();

    for request in server.incoming_requests() {
        println!("received request! method: {}, url: {}, headers: {}",
            request.get_method(),
            request.get_url(),
            request.get_headers()
        );

        let response = tiny_http::Response::from_string("hello world".to_string());
        request.respond(response);
    }
}
