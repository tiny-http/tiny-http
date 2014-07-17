extern crate httpd = "tiny-http";

fn main() {
    let server = httpd::Server::new().unwrap();

    for request in server.incoming_requests() {
        println!("received request! method: {}, url: {}, headers: {}",
            request.get_method(),
            request.get_url(),
            request.get_headers()
        );

        let response = httpd::Response::from_string("hello world".to_string());
        request.respond(response);
    }
}
