#![feature(phase)]

extern crate httpd = "tiny-http";

use std::io::Command;

#[test]
#[ignore]
// TODO: obtain time
fn curl_bench() {
    let (server, port) = httpd::Server::new_with_random_port().unwrap();
    let num_requests = 10u;

    match Command::new("curl")
        .arg("-s")
        .arg(format!("http://localhost:{}/?[1-{}]", port, num_requests).as_slice())
        .output()
    {
        Ok(p) => p,
        Err(err) => return,       // ignoring test
    };
}
