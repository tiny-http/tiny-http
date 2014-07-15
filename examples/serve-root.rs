extern crate httpd = "tiny-http";

fn get_content_type(path: &Path) -> &'static str {
    let extension = match path.extension_str() {
        None => return "text/plain",
        Some(e) => e
    };

    match extension {
        "gif" => "image/gif",
        "jpg" => "image/jpeg",
        "jpeg" => "image/jpeg",
        "png" => "image/png",
        "pdf" => "application/pdf",
        "htm" => "text/html",
        "html" => "text/html",
        _ => "text/plain"
    }
}

fn main() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    println!("Now listening on port {}", port);

    loop {
        let rq = match server.recv() {
            Ok(rq) => rq,
            Err(_) => break
        };

        println!("{}", rq);

        let path = Path::new(rq.get_url().path.clone());
        let file = std::io::File::open(&path);

        if file.is_ok() {
            let response = httpd::Response::from_file(file.unwrap());

            let response = response.with_header(
                httpd::Header{
                    field: from_str("Content-Type").unwrap(),
                    value: get_content_type(&path).to_string()
                }
            );

            rq.respond(response);

        } else {
            let rep = httpd::Response::empty().with_status_code(httpd::StatusCode(404));
            rq.respond(rep);
        }
    }
}
