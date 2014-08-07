extern crate httpd = "tiny-http";

use std::os;
use std::sync::Arc;

fn main() {
    let server = Arc::new(httpd::ServerBuilder::new().with_port(9975).build().unwrap());
    println!("Now listening on port 9975");

    for _ in range(0, os::num_cpus()) {
        let server = server.clone();

        spawn(proc() {
            for rq in server.incoming_requests() {
                let response = httpd::Response::from_string("hello world".to_string());
                rq.respond(response);
            }
        })
    }
}
