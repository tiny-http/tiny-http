extern crate httpd = "tiny-http";

fn main() {
    let mut server = httpd::Server::new().unwrap();

    loop {
        let mut rq = server.recv();

        println!("Request: {} {}", rq.get_method(), rq.get_url());

        let response = httpd::Response::from_string(
            format!("Method: {}\nURL: {}\nHeaders: {}", rq.get_method(),
            rq.get_url(), rq.get_headers()));
        rq.respond(response);
    }
}
