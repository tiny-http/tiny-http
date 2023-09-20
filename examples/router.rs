use tiny_http::{Server, Response, Request};
use std::{collections::HashMap, io::Cursor};

type RouteHandler = fn(&mut Request) -> Response<Cursor<Vec<u8>>>;

fn get_root(_req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("You've reached the root!")
}

fn get_hello(_req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("Hello from /hello!")
}

fn post_echo(req: &mut Request) -> Response<Cursor<Vec<u8>>> {
    let mut request_body_bytes = Vec::new();
    req.as_reader().read_to_end(&mut request_body_bytes).unwrap();
    let request_body_string = String::from_utf8(request_body_bytes.clone()).unwrap();
    Response::from_data(request_body_string.as_bytes())
}

fn main() {
    let routes = HashMap::from([
        ("GET:/".to_string(), get_root as RouteHandler),
        ("GET:/hello".to_string(), get_hello as RouteHandler),
        ("POST:/echo".to_string(), post_echo as RouteHandler),
    ]);
    let server = Server::http("0.0.0.0:3000").unwrap();
    for mut request in server.incoming_requests() {
        let route_key = format!("{}:{}", request.method(), request.url());
        match routes.get(&route_key) {
            Some(handler) => {
                let response = handler(&mut request);
                request.respond(response).unwrap();
            },
            None => {
                let response = Response::from_string("404 Not Found").with_status_code(404);
                request.respond(response).unwrap();
            }
        }
    }
}
