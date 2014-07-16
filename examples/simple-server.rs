extern crate httpd = "tiny-http";

fn main() {
    let server = httpd::Server::new_with_port(9975).unwrap();
    println!("Now listening on port 9975");

    for rq in server.incoming_requests() {
        let response = httpd::Response::from_string("hello world".to_string());
        rq.respond(response);
    }
}
