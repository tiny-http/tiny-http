extern crate rustc_serialize;
extern crate sha1;
extern crate tiny_http;

use std::io::Cursor;
use std::io::Read;
use std::thread::spawn;

use http::{header, HeaderValue};
use rustc_serialize::base64::{Config, Newline, Standard, ToBase64};

fn home_page(port: u16) -> tiny_http::Response<Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(format!(
        "
        <script type=\"text/javascript\">
        var socket = new WebSocket(\"ws://localhost:{}/\", \"ping\");

        function send(data) {{
            socket.send(data);
        }}

        socket.onmessage = function(event) {{
            document.getElementById('result').innerHTML += event.data + '<br />';
        }}
        </script>
        <p>This example will receive &quot;Hello&quot; for each byte in the packet being sent.
        Tiny-http doesn't support decoding websocket frames, so we can't do anything better.</p>
        <p><input type=\"text\" id=\"msg\" />
        <button onclick=\"send(document.getElementById('msg').value)\">Send</button></p>
        <p>Received: </p>
        <p id=\"result\"></p>
    ",
        port
    ))
    .with_header(header::CONTENT_TYPE, HeaderValue::from_static("text/html"))
}

/// Turns a Sec-WebSocket-Key into a Sec-WebSocket-Accept.
/// Feel free to copy-paste this function, but please use a better error handling.
fn convert_key(input: &str) -> String {
    use sha1::Sha1;

    let mut input = input.to_string().into_bytes();
    let mut bytes = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
        .to_string()
        .into_bytes();
    input.append(&mut bytes);

    let mut sha1 = Sha1::new();
    sha1.update(&input);

    sha1.digest().bytes().to_base64(Config {
        char_set: Standard,
        pad: true,
        line_length: None,
        newline: Newline::LF,
    })
}

fn main() {
    let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();

    println!("Server started");
    println!(
        "To try this example, open a browser to http://localhost:{}/",
        port
    );

    for request in server.incoming_requests() {
        // we are handling this websocket connection in a new task
        spawn(move || {
            // checking the "Upgrade" header to check that it is a websocket
            if request.headers().get(header::UPGRADE)
                == Some(&HeaderValue::from_static("websocket"))
            {
                // sending the HTML page
                request.respond(home_page(port)).expect("Responded");
                return;
            }

            // getting the value of Sec-WebSocket-Key
            let key = match request
                .headers()
                .get(header::SEC_WEBSOCKET_KEY)
                .and_then(|value| value.to_str().ok())
            {
                None => {
                    let response = tiny_http::Response::new_empty(http::StatusCode::BAD_REQUEST);
                    request.respond(response).expect("Responded");
                    return;
                }
                Some(k) => k,
            };

            // building the "101 Switching Protocols" response
            let response = tiny_http::Response::new_empty(http::StatusCode::SWITCHING_PROTOCOLS)
                .with_header(header::UPGRADE, HeaderValue::from_static("websocket"))
                .with_header(header::CONNECTION, header::UPGRADE.into())
                .with_header(
                    header::SEC_WEBSOCKET_PROTOCOL,
                    HeaderValue::from_static("ping"),
                )
                .with_header(
                    header::SEC_WEBSOCKET_ACCEPT,
                    convert_key(key).parse().unwrap(),
                );

            //
            let mut stream = request.upgrade("websocket", response);

            //
            loop {
                let mut out = Vec::new();
                match Read::by_ref(&mut stream).take(1).read_to_end(&mut out) {
                    Ok(n) if n >= 1 => {
                        // "Hello" frame
                        let data = [0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f];
                        stream.write(&data).ok();
                        stream.flush().ok();
                    }
                    Ok(_) => panic!("eof ; should never happen"),
                    Err(e) => {
                        println!("closing connection because: {}", e);
                        return;
                    }
                };
            }
        });
    }
}
