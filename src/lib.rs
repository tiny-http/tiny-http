#![crate_name = "tiny-http"]
#![crate_type = "lib"]
#![license = "Apache"]

extern crate semver;
extern crate url;

use std::io::{Acceptor, BufferedReader, IoResult, Listener, RefReader, RefWriter};
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
    body_length: uint,
}

pub struct ResponseWriter {
    writer: tcp::TcpStream
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
                if attempt.is_ok() {       // TODO: remove the channel if it is closed
                    return Some(attempt.unwrap());
                }
            }
        }

        None
    }

    /// Does not block.
    fn process_new_clients(&self) {
        let mut new_clients = Vec::new();

        // we add all the elements available on connections_receiver to new_clients
        loop {
            match self.connections_receiver.try_recv() {
                Ok(client) => new_clients.push(client),
                Err(_) => break
            }
        }

        // for each new client, spawning a task that will
        // continuously try to read a Request
        for client in new_clients.move_iter().filter_map(|c| c.ok()) {
            let (tx, rx) = channel();
            spawn(proc() {
                let mut client = client;
                // TODO: when the channel is being closed, immediatly notify the task
                client.advance(|rq| tx.send_opt(rq).is_ok());
            });
            self.requests_receiver.lock().push(rx);
        }
    }
}

impl Request {
    /// Returns the method requested by the client (eg. GET, POST, etc.)
    pub fn get_method<'a>(&'a self) -> &'a Method {
        &self.method
    }

    /// Returns the resource requested by the client.
    pub fn get_url<'a>(&'a self) -> &'a url::Path {
        &self.path
    }

    /// Returns a list of all headers sent by the client.
    pub fn get_headers<'a>(&'a self) -> &'a [Header] {
        self.headers.as_slice()
    }

    /// Returns the length of the body in bytes.
    pub fn get_body_length(&self) -> uint {
        self.body_length
    }

    /// Allows to read the body of the request.
    pub fn as_reader<'a>(&'a mut self)
        -> RefReader<'a, LimitReader<BufferedReader<tcp::TcpStream>>>
    {
        fn as_reader_impl<'a, R: Reader>(elem: &'a mut R) -> RefReader<'a, R> {
            elem.by_ref()
        }
        as_reader_impl(&mut self.read_socket)
    }

    /// Turns the Request into a writer.
    /// The writer has a raw access to the stream to the user.
    /// This function is useful for things like CGI.
    pub fn into_writer(self) -> ResponseWriter {
        Request::finish_reading(self.read_socket);

        ResponseWriter { writer: self.write_socket }
    }

    /// Sends a response to this request.
    pub fn respond<R: Reader>(self, response: Response<R>) {
        Request::finish_reading(self.read_socket);

        match response.raw_print(self.write_socket) {
            Ok(_) => (),
            Err(err) => println!("error while sending answer: {}", err)     // TODO: handle better?
        }
    }

    /// Consumes the rest of the request's body in the TcpStream.
    fn finish_reading(reader: LimitReader<BufferedReader<tcp::TcpStream>>) {
        let remaining_to_read = reader.limit();
        let underlying = reader.unwrap().consume(remaining_to_read);
    }
}

impl Writer for ResponseWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.writer.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }

    #[inline]
    fn write_fmt(&mut self, fmt: &::std::fmt::Arguments) -> IoResult<()> {
        self.writer.write_fmt(fmt)
    }

    #[inline]
    fn write_str(&mut self, s: &str) -> IoResult<()> {
        self.writer.write_str(s)
    }

    #[inline]
    fn write_line(&mut self, s: &str) -> IoResult<()> {
        self.writer.write_line(s)
    }

    #[inline]
    fn write_char(&mut self, c: char) -> IoResult<()> {
        self.writer.write_char(c)
    }

    #[inline]
    fn write_int(&mut self, n: int) -> IoResult<()> {
        self.writer.write_int(n)
    }

    #[inline]
    fn write_uint(&mut self, n: uint) -> IoResult<()> {
        self.writer.write_uint(n)
    }

    #[inline]
    fn write_le_uint(&mut self, n: uint) -> IoResult<()> {
        self.writer.write_le_uint(n)
    }

    #[inline]
    fn write_le_int(&mut self, n: int) -> IoResult<()> {
        self.writer.write_le_int(n)
    }

    #[inline]
    fn write_be_uint(&mut self, n: uint) -> IoResult<()> {
        self.writer.write_be_uint(n)
    }

    #[inline]
    fn write_be_int(&mut self, n: int) -> IoResult<()> {
        self.writer.write_be_int(n)
    }

    #[inline]
    fn write_be_u64(&mut self, n: u64) -> IoResult<()> {
        self.writer.write_be_u64(n)
    }

    #[inline]
    fn write_be_u32(&mut self, n: u32) -> IoResult<()> {
        self.writer.write_be_u32(n)
    }

    #[inline]
    fn write_be_u16(&mut self, n: u16) -> IoResult<()> {
        self.writer.write_be_u16(n)
    }

    #[inline]
    fn write_be_i64(&mut self, n: i64) -> IoResult<()> {
        self.writer.write_be_i64(n)
    }

    #[inline]
    fn write_be_i32(&mut self, n: i32) -> IoResult<()> {
        self.writer.write_be_i32(n)
    }

    #[inline]
    fn write_be_i16(&mut self, n: i16) -> IoResult<()> {
        self.writer.write_be_i16(n)
    }

    #[inline]
    fn write_be_f64(&mut self, f: f64) -> IoResult<()> {
        self.writer.write_be_f64(f)
    }

    #[inline]
    fn write_be_f32(&mut self, f: f32) -> IoResult<()> {
        self.writer.write_be_f32(f)
    }

    #[inline]
    fn write_le_u64(&mut self, n: u64) -> IoResult<()> {
        self.writer.write_le_u64(n)
    }

    #[inline]
    fn write_le_u32(&mut self, n: u32) -> IoResult<()> {
        self.writer.write_le_u32(n)
    }

    #[inline]
    fn write_le_u16(&mut self, n: u16) -> IoResult<()> {
        self.writer.write_le_u16(n)
    }

    #[inline]
    fn write_le_i64(&mut self, n: i64) -> IoResult<()> {
        self.writer.write_le_i64(n)
    }

    #[inline]
    fn write_le_i32(&mut self, n: i32) -> IoResult<()> {
        self.writer.write_le_i32(n)
    }

    #[inline]
    fn write_le_i16(&mut self, n: i16) -> IoResult<()> {
        self.writer.write_le_i16(n)
    }

    #[inline]
    fn write_le_f64(&mut self, f: f64) -> IoResult<()> {
        self.writer.write_le_f64(f)
    }

    #[inline]
    fn write_le_f32(&mut self, f: f32) -> IoResult<()> {
        self.writer.write_le_f32(f)
    }

    #[inline]
    fn write_u8(&mut self, n: u8) -> IoResult<()> {
        self.writer.write_u8(n)
    }

    #[inline]
    fn write_i8(&mut self, n: i8) -> IoResult<()> {
        self.writer.write_i8(n)
    }
}

impl std::fmt::Show for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter)
        -> Result<(), std::fmt::FormatError>
    {
        (format!("Request({} {})",
            self.method, self.path)).fmt(formatter)
    }
}
