use tiny_http::{Server, Response};

fn main() {
    let server = Server::http("0.0.0.0:8000").unwrap();

    for request in server.incoming_requests() {
        let mut request = request;

        match request.to_string() {
            Ok(s) => {
                println!("{}", s);
                request.respond(Response::from_string(s)).unwrap()
            }
            Err(e) => request.respond(Response::from_string(format!("Couldn't upload data: {}", e))).unwrap()
        }
    }
}