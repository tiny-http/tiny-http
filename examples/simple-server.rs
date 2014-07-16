extern crate httpd = "tiny-http";

fn main() {
    let server = httpd::Server::new_with_port(9975).unwrap();
    println!("Now listening on port 9975");

    loop {
        let rq = match server.recv() {
            Ok(rq) => rq,
            Err(_) => break
        };

        let response = httpd::Response::from_string("hello world".to_string());
        rq.respond(response);
    }
}
