extern crate httpd = "tiny-http";

fn main() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    println!("Now listening on port {}", port);

    loop {
        let rq = match server.recv() {
            Ok(rq) => rq,
            Err(_) => break
        };

        println!("{}", rq);

        let response = httpd::Response::from_string(
            format!("Method: {}\nURL: {}\nHeaders: {}", rq.get_method(),
            rq.get_url(), rq.get_headers()));
        rq.respond(response);
    }
}
