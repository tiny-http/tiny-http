extern crate crypto;
extern crate serialize;
extern crate tiny_http;

use std::io::MemReader;

fn home_page(port: u16) -> tiny_http::Response<MemReader> {
    tiny_http::Response::from_string(format!("
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
    ", port))
        .with_header(from_str("Content-type: text/html").unwrap())
}

/// Turns a Sec-WebSocket-Key into a Sec-WebSocket-Accept.
/// Feel free to copy-paste this function, but please use a better error handling.
fn convert_key(input: &str) -> String {
    use serialize::base64::{Config, Standard, ToBase64};
    use crypto::digest::Digest;
    use crypto::sha1::Sha1;

    let input = input.to_string().into_bytes();
    let input = input.append("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".to_string().into_bytes().as_slice());

    let mut sha1 = Sha1::new();
    sha1.input(input.as_slice());

    let mut out = [0u8, ..20];
    sha1.result(out);

    out.as_slice().to_base64(Config{char_set: Standard, pad: true, line_length: None})
}

fn main() {
    let server = tiny_http::ServerBuilder::new().with_random_port().build().unwrap();
    let port = server.get_server_addr().port;

    println!("Server started");
    println!("To try this example, open a browser to http://localhost:{}/", port);

    for request in server.incoming_requests() {
        // we are handling this websocket connection in a new task
        spawn(move || {
            use std::ascii::{AsciiCast, AsciiStr};

            // checking the "Upgrade" header to check that it is a websocket
            match request.get_headers().iter()
                .find(|h| h.field.equiv(&"Upgrade")) 
                .filtered(|hdr| hdr.value.as_slice().eq_ignore_case(b"websocket".to_ascii()))
            {
                None => {
                    // sending the HTML page
                    request.respond(home_page(port));
                    return
                },
                _ => ()
            };

            // getting the value of Sec-WebSocket-Key
            let key = match request.get_headers().iter()
                .find(|h| h.field.equiv(&"Sec-WebSocket-Key"))
            {
                None => {
                    let response = tiny_http::Response::new_empty(tiny_http::StatusCode(400));
                    request.respond(response);
                    return
                },
                Some(h) => &h.value
            };

            // building the "101 Switching Protocols" response
            let response = tiny_http::Response::new_empty(tiny_http::StatusCode(101))
                .with_header(from_str("Upgrade: websocket").unwrap())
                .with_header(from_str("Connection: Upgrade").unwrap())
                .with_header(from_str("Sec-WebSocket-Protocol: ping").unwrap())
                .with_header(from_str(format!("Sec-WebSocket-Accept: {}",
                    convert_key(key.as_slice().as_str_ascii())).as_slice()).unwrap());

            // 
            let mut stream = request.upgrade("websocket", response);

            // 
            loop {
                match stream.read_byte() {
                    Ok(_) => {
                        // "Hello" frame
                        let data = [0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f];
                        stream.write(data.as_slice()).ok();
                        stream.flush().ok();
                    },
                    Err(e) => {
                        println!("closing connection because: {}", e);
                        return
                    }
                };
            }
        });
    }
}
