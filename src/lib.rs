#![crate_name = "tiny-http"]
#![crate_type = "lib"]
#![license = "Apache"]

extern crate semver;
extern crate url;

use std::io::{Acceptor, BufferedReader, IoResult, Listener, RefWriter};
use std::io::net::ip;
use std::io::net::tcp;
use std::io::util::LimitReader;
use std::sync;

pub use common::Header;
pub use common::Method;
pub use common::StatusCode;
pub use response::Response;

mod client;
mod common;
mod response;

/// The main class of this library.
/// Create a new server using `Server::new()`.
pub struct Server {
    connections_receiver: Receiver<IoResult<client::ClientConnection>>,
    requests_receiver: sync::Mutex<Vec<Receiver<Request>>>,
}

/// Represents an HTTP request made by a client.
pub struct Request {
    read_socket: LimitReader<BufferedReader<tcp::TcpStream>>,
    write_socket: tcp::TcpStream,
    method: Method,
    path: url::Path,
    http_version: semver::Version,
    headers: Vec<Header>,
}

impl Server {
    /// Builds a new server on port 80 that listens to all inputs.
    pub fn new() -> IoResult<Server> {
        Server::new_with_port(80)
    }

    /// Builds a new server on a given port and that listens to all inputs.
    pub fn new_with_port(port: ip::Port) -> IoResult<Server> {
        Server::new_with_addr(&ip::SocketAddr{ip: ip::Ipv4Addr(0, 0, 0, 0), port: port})
    }

    /// Builds a new server that listens on the specified address.
    pub fn new_with_addr(addr: &ip::SocketAddr) -> IoResult<Server> {
        // building the TcpAcceptor
        let server = try!(tcp::TcpListener::bind(
            format!("{}", addr.ip).as_slice(), addr.port).listen());

        // creating a task where server.accept() is continuously called
        // and ClientConnection objects are returned in the receiver
        let (tx, rx) = channel();
        spawn(proc() {
            let mut server = server;

            loop {
                let val = server.accept().map(|sock| client::ClientConnection::new(sock));

                match tx.send_opt(val) {
                    Err(_) => break,
                    _ => ()
                }
            }
        });

        // result
        Ok(Server {
            connections_receiver: rx,
            requests_receiver: sync::Mutex::new(Vec::new()),
        })
    }

    /// Returns the number of clients currently connected to the server.
    pub fn get_num_connections(&self) -> uint {
        self.requests_receiver.lock().len()
    }

    /// Blocks until an HTTP request has been submitted and returns it.
    pub fn recv(&self) -> Request {
        loop {
            match self.try_recv() {
                Some(rq) => return rq,
                None => ()
            };
        }
    }

    /// Same as `recv()` but doesn't block.
    pub fn try_recv(&self) -> Option<Request> {
        self.process_new_clients();

        // TODO: rewrite this
        {
            let mut locked_receivers = self.requests_receiver.lock();
            for rx in locked_receivers.iter() {
                let attempt = rx.try_recv();
                if attempt.is_ok() {
                    return Some(attempt.unwrap());
                }
            }
        }

        None
    }

    /// Does not block.
    fn process_new_clients(&self) {
        let mut new_clients = Vec::new();

        loop {
            match self.connections_receiver.try_recv() {
                Ok(client) => new_clients.push(client),
                Err(_) => break
            }
        }

        for client in new_clients.move_iter().filter_map(|c| c.ok()) {
            let (tx, rx) = channel();
            spawn(proc() {
                let mut client = client;
                client.advance(|rq| tx.send_opt(rq).is_ok());
            });
            self.requests_receiver.lock().push(rx);
        }
    }
}

impl Request {
    pub fn get_method<'a>(&'a self) -> &'a Method {
        &self.method
    }

    pub fn get_url<'a>(&'a self) -> &'a url::Path {
        &self.path
    }

    pub fn get_header<'a>(&'a self, name: &str) -> Option<&'a str> {
        for header in self.headers.iter() {
            if header.field.equiv(&name) {
                return Some(header.value.as_slice());
            }
        }

        None
    }

    pub fn get_headers<'a>(&'a self) -> &'a [Header] {
        self.headers.as_slice()
    }

    pub fn as_raw_writer<'a>(&'a mut self) -> RefWriter<'a, tcp::TcpStream> {
        Request::as_raw_writer_impl(&mut self.write_socket)
    }

    fn as_raw_writer_impl<'a, W: Writer>(elem: &'a mut W) -> RefWriter<'a, W> {
        elem.by_ref()
    }

    pub fn respond<R: Reader>(self, response: Response<R>) {
        match response.raw_print(self.write_socket) {
            Ok(_) => (),
            Err(err) => println!("error while sending answer: {}", err)     // TODO: handle better?
        }
    }
}

impl std::fmt::Show for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter)
        -> Result<(), std::fmt::FormatError>
    {
        (format!("Request {{ method: {}, path: {}, http_version: {}, headers: {} }}",
            self.method, self.path, self.http_version, self.headers)).fmt(formatter)
    }
}
