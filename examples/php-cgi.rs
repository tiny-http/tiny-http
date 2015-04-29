extern crate tiny_http;

/**!

A web server that redirects every request to a PHP script.

Usage: php-cgi <php-script-path>

*/

use std::os;

fn handle(rq: tiny_http::Request, script: &str) {
    use std::process::{Command, ExitStatus};
    use std::io::util;
    use std::io::Write;

    let mut php = Command::new("php-cgi")
        .arg(script)
        //.stdin(Ignored)
        //.extra_io(Ignored)
        .env("AUTH_TYPE", "")
        .env("CONTENT_LENGTH", format!("{}", rq.get_body_length().unwrap_or(0)))
        .env("CONTENT_TYPE", "")
        .env("GATEWAY_INTERFACE", "CGI/1.1")
        .env("PATH_INFO", "")
        .env("PATH_TRANSLATED", "")
        .env("QUERY_STRING", format!("{}", rq.get_url()))
        .env("REMOTE_ADDR", format!("{}", rq.get_remote_addr().ip))
        .env("REMOTE_HOST", "")
        .env("REMOTE_IDENT", "")
        .env("REMOTE_USER", "")
        .env("REQUEST_METHOD", format!("{}", rq.get_method()))
        .env("SCRIPT_NAME", script)
        .env("SERVER_NAME", "tiny-http php-cgi example")
        .env("SERVER_PORT", format!("{}", rq.get_remote_addr().port))
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("SERVER_SOFTWARE", "tiny-http php-cgi example")
        .output()
        .unwrap();


    // note: this is not a good implementation
    // cgi returns the status code in the headers ; also many headers will be missing
    //  in the response
    match php.status {
        status if status.success() => {
            let mut writer = rq.into_writer();
            let mut writer: &mut Write = &mut *writer;

            (write!(writer, "HTTP/1.1 200 OK\r\n")).unwrap();
            (write!(writer, "{}", php.output.into_ascii().into_string())).unwrap();

            writer.flush().unwrap();
        },
        _ => {
            println!("Error in script execution: {}", php.error.clone().into_ascii().into_string());
        }
    }
}

fn main() {
    use std::sync::Arc;
    use std::thread::spawn;

    let php_script = {
        let args = os::args();
        if args.len() < 2 { println!("Usage: php-cgi <php-script-path>"); return }
        args[1].to_string()
    };

    let server = Arc::new(tiny_http::ServerBuilder::new().with_port(9975).build().unwrap());
    println!("Now listening on port 9975");

    for _ in 0..os::num_cpus() {
        let server = server.clone();
        let php_script = php_script.clone();

        spawn(move || {
            for rq in server.incoming_requests() {
                handle(rq, php_script.as_slice());
            }
        })
    }
}
