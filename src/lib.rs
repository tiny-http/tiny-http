/*!
# Simple usage

## Creating the server

The easiest way to create a server is to call `Server::new()`.

The `new()` function returns an `IoResult<Server>` which will return an error
in the case where the server creation fails (for example if the listening port is already
occupied).

```rust
let server = httpd::Server::new().unwrap();
```

A newly-created `Server` will immediatly start listening for incoming connections and HTTP
requests.

## Receiving requests

Calling `server.recv()` will block until the next request is available.
This function returns an `IoResult<Request>`, so you need to handle the possible errors.

```rust
loop {
    // blocks until the next request is received
    let request = match server.recv() {
        Ok(rq) => rq,
        Err(e) => { println!("error: {}", e); break }
    };

    // do something with the request
    ...
}
```

In a real-case scenario, you will probably want to spawn multiple worker tasks and call
`server.recv()` on all of them. Like this:

```rust
let server = Arc::new(server);

for _ in range(0u, 4) {
    spawn(proc() {
        loop {
            let rq = server.recv().unwrap();

            ...
        }
    })
}
```

If you don't want to block, you can call `server.try_recv()` instead.

## Handling requests

The `Request` object returned by `server.recv()` contains informations about the client's request.
The most useful methods are probably `request.get_method()` and `request.get_url()` which return
the requested method (`GET`, `POST`, etc.) and url.

To handle a request, you need to create a `Response` object. See the docs of this object for
more infos. Here is an example of creating a `Response` from a file:

```rust
let response = httpd::Response::from_file(Path::new("image.png"));
```

All that remains to do is call `request.respond()`:

```rust
request.respond(response)
```
*/

#![crate_name = "tiny-http"]
#![crate_type = "lib"]
#![license = "Apache"]
#![feature(unsafe_destructor)]

extern crate encoding;
extern crate time;
extern crate url;

use std::io::{Acceptor, IoResult, Listener};
use std::io::net::ip;
use std::io::net::tcp;
use std::comm::Select;
use std::sync::{Arc, Mutex};
use std::sync::atomics::AtomicBool;
use client::ClientConnection;

pub use common::{Header, HeaderField, HTTPVersion, Method, StatusCode};
pub use response::Response;

mod client;
mod common;
mod response;

#[allow(dead_code)]     // TODO: remove when everything is implemented
mod util;

/// The main class of this library.
/// 
/// Destroying this object will immediatly close the listening socket annd the reading
///  part of all the client's connections. Requests that have already been returned by
///  the `recv()` function will not close and the responses will be transferred to the client.
#[unstable]
pub struct Server {
    // tasks where the client connections are dispatched
    tasks_pool: util::TaskPool,

    // receiver for client connections
    connections_receiver: Mutex<Receiver<IoResult<ClientConnection>>>,

    // should be false as long as the server exists
    // when set to true, all the subtasks will close within a few hundreds ms
    close: Arc<AtomicBool>,

    // the sender linked to requests_receiver
    // cloned each time a client connection is created
    requests_sender: Mutex<Sender<Request>>,

    // channel to receive requests from
    requests_receiver: Mutex<Receiver<Request>>,

    // result of TcpListener::socket_name()
    listening_addr: ip::SocketAddr,
}

// this trait is to make sure that Server implements Share and Send
trait MustBeShareDummy : Share + Send {}
impl MustBeShareDummy for Server {}

#[unstable]
pub struct IncomingRequests<'a> {
    server: &'a Server
}

/// Represents an HTTP request made by a client.
///
/// A `Request` object is what is produced by the server, and is your what
///  your code must analyse and answer.
///
/// This object implements the `Send` trait, therefore you can dispatch your requests to
///  worker threads.
///
/// It is possible that multiple requests objects are simultaneously linked to the same client,
///  but don't worry: tiny-http automatically handles synchronization of the answers.
///
/// If a `Request` object is destroyed without `into_writer` or `respond` being called,
///  an empty response with a 500 status code (internal server error) will automatically be
///  sent back to the client.
/// This means that if your code fails during the handling of a request, this "internal server
///  error" response will automatically be sent during the stack unwinding.
#[unstable]
pub struct Request {
    // where to read the body from
    data_reader: Box<Reader + Send>,

    // if this writer is empty, then the request has been answered
    response_writer: Option<Box<Writer + Send>>,

    remote_addr: ip::SocketAddr,

    method: Method,

    path: url::Path,

    http_version: HTTPVersion,

    headers: Vec<Header>,

    body_length: Option<uint>,
}

// this trait is to make sure that Request implements Send
trait MustBeSendDummy : Send {}
impl MustBeSendDummy for Request {}

impl Server {
    /// Builds a new server on port 80 that listens to all inputs.
    #[unstable]
    pub fn new() -> IoResult<Server> {
        Server::new_with_port(80)
    }

    /// Builds a new server on a given port and that listens to all inputs.
    #[unstable]
    pub fn new_with_port(port: ip::Port) -> IoResult<Server> {
        Server::new_with_addr(&ip::SocketAddr{ip: ip::Ipv4Addr(0, 0, 0, 0), port: port})
    }

    /// Builds a new server on a rand port and that listens to all inputs.
    /// Returns the server and the port it was created on.
    /// This function is guaranteed not to fail because of a port already in use,
    ///  and is useful for testing purposes.
    #[unstable]
    pub fn new_with_random_port() -> IoResult<(Server, ip::Port)> {
        Server::new_with_addr(&ip::SocketAddr{ip: ip::Ipv4Addr(0, 0, 0, 0), port: 0})
            .map(|s| { let port = s.get_server_addr().port; (s, port) })
    }

    /// Builds a new server that listens on the specified address.
    #[unstable]
    pub fn new_with_addr(addr: &ip::SocketAddr) -> IoResult<Server> {
        // building the "close" variable
        let close_trigger = Arc::new(AtomicBool::new(false));

        // building the TcpAcceptor
        let (server, local_addr) = {
            let mut listener = try!(tcp::TcpListener::bind(
                format!("{}", addr.ip).as_slice(), addr.port));
            let local_addr = try!(listener.socket_name());
            let server = try!(listener.listen());
            let server = util::ClosableTcpAcceptor::new(server, close_trigger.clone());
            (server, local_addr)
        };

        // creating a task where server.accept() is continuously called
        // and ClientConnection objects are returned in the receiver
        let (tx_incoming, rx_incoming) = channel();

        let inside_close_trigger = close_trigger.clone();
        spawn(proc() {
            let mut server = server;

            loop {
                let new_client = server.accept().map(|sock| {
                    use util::ClosableTcpStream;

                    let read_closable = ClosableTcpStream::new(sock.clone(),
                        inside_close_trigger.clone(), true, false);

                    let write_closable = ClosableTcpStream::new(sock.clone(),
                        inside_close_trigger.clone(), false, true);

                    ClientConnection::new(write_closable, read_closable)
                });

                if tx_incoming.send_opt(new_client).is_err() {
                    break
                }
            }
        });

        // 
        let (tx_requests, rx_requests) = channel();

        // result
        Ok(Server {
            tasks_pool: util::TaskPool::new(),
            connections_receiver: Mutex::new(rx_incoming),
            close: close_trigger,
            requests_sender: Mutex::new(tx_requests),
            requests_receiver: Mutex::new(rx_requests),
            listening_addr: local_addr,
        })
    }

    /// Returns an iterator for all the incoming requests.
    ///
    /// The iterator will return `None` if the server socket is shutdown.
    #[unstable]
    pub fn incoming_requests<'a>(&'a self) -> IncomingRequests<'a> {
        IncomingRequests { server: self }
    }

    /// Returns the address the server is listening to.
    #[experimental]
    pub fn get_server_addr(&self) -> ip::SocketAddr {
        self.listening_addr.clone()
    }

    /// Returns the number of clients currently connected to the server.
    #[stable]
    pub fn get_num_connections(&self) -> uint {
        unimplemented!()
        //self.requests_receiver.lock().len()
    }

    /// Blocks until an HTTP request has been submitted and returns it.
    #[stable]
    pub fn recv(&self) -> IoResult<Request> {
        let connections_receiver = self.connections_receiver.lock();
        let requests_receiver = self.requests_receiver.lock();

        // TODO: the select! macro doesn't seem to be usable without moving
        //       out of self, so we use Select directly

        let select = Select::new();

        let mut request_handle = select.handle(&*requests_receiver);
        unsafe { request_handle.add() };

        let mut connect_handle = select.handle(&*connections_receiver);
        unsafe { connect_handle.add() };

        loop {
            let id = select.wait();

            if id == request_handle.id() {
                let request = request_handle.recv();

                unsafe { request_handle.remove() };
                unsafe { connect_handle.remove() };

                return Ok(request)
            }

            if id == connect_handle.id() {
                let client = connect_handle.recv_opt();

                match client {
                    Ok(Ok(client)) => {
                        self.add_client(client);
                        continue
                    },
                    Ok(Err(err)) => {
                        unsafe { request_handle.remove() };
                        unsafe { connect_handle.remove() };

                        return Err(err)
                    },
                    Err(_) => {
                        use std::io;

                        unsafe { request_handle.remove() };
                        unsafe { connect_handle.remove() };

                        return Err(io::standard_error(io::Closed));
                    }
                }
            }

            unreachable!()
        }
    }

    /// Same as `recv()` but doesn't block.
    #[stable]
    pub fn try_recv(&self) -> IoResult<Option<Request>> {
        let mut connections_receiver = self.connections_receiver.lock();
        let mut requests_receiver = self.requests_receiver.lock();

        // processing all new clients
        loop {
            match connections_receiver.try_recv() {
                Ok(client) => self.add_client(try!(client)),
                Err(_) => break
            }
        }

        // reading the next request
        Ok(requests_receiver.try_recv().ok())
    }

    /// Adds a new client to the list.
    fn add_client(&self, client: ClientConnection) {
        let requests_sender = self.requests_sender.lock().clone();

        self.tasks_pool.spawn(proc() {
            let mut client = client;
            // TODO: when the channel is being closed, immediatly notify the task
            client.advance(|rq| requests_sender.send_opt(rq).is_ok());
        });
    }
}

impl Request {
    /// Returns the method requested by the client (eg. `GET`, `POST`, etc.).
    #[stable]
    pub fn get_method<'a>(&'a self) -> &'a Method {
        &self.method
    }

    /// Returns the resource requested by the client.
    #[unstable]
    pub fn get_url<'a>(&'a self) -> &'a url::Path {
        &self.path
    }

    /// Returns a list of all headers sent by the client.
    #[stable]
    pub fn get_headers<'a>(&'a self) -> &'a [Header] {
        self.headers.as_slice()
    }

    /// Returns the HTTP version of the request.
    #[unstable]
    pub fn get_http_version<'a>(&'a self) -> &'a HTTPVersion {
        &self.http_version
    }

    /// Returns the length of the body in bytes.
    ///
    /// Returns `None` if the length is unknown.
    #[unstable]
    pub fn get_body_length(&self) -> Option<uint> {
        self.body_length
    }

    /// Returns the length of the body in bytes.
    #[stable]
    pub fn get_remote_addr<'a>(&'a self) -> &'a ip::SocketAddr {
        &self.remote_addr
    }

    /// Allows to read the body of the request.
    /// 
    /// # Example
    /// 
    /// ```
    /// let request = server.recv();
    /// 
    /// if get_content_type(&request) == "application/json" {
    ///     let json: Json = from_str(request.as_reader().read_to_string()).unwrap();
    /// }
    /// ```
    #[unstable]
    pub fn as_reader<'a>(&'a mut self) -> &'a mut Reader {
        fn passthrough<'a>(r: &'a mut Reader) -> &'a mut Reader { r }
        passthrough(self.data_reader)
    }

    /// Turns the `Request` into a writer.
    /// 
    /// The writer has a raw access to the stream to the user.
    /// This function is useful for things like CGI.
    ///
    /// Note that the destruction of the `Writer` object may trigger
    ///  some events. For exemple if a client has sent multiple requests and the requests
    ///  have been processed in parallel, the destruction of a writer will trigger
    ///  the writing of the next response.
    /// Therefore you should always destroy the `Writer` as soon as possible.
    #[stable]
    pub fn into_writer(mut self) -> Box<Writer + Send> {
        self.into_writer_impl()
    }

    fn into_writer_impl(&mut self) -> Box<Writer + Send> {
        use std::mem;

        assert!(self.response_writer.is_some());

        let mut writer = None;
        mem::swap(&mut self.response_writer, &mut writer);
        writer.unwrap()
    }

    /// Sends a response to this request.
    #[unstable]
    pub fn respond<R: Reader>(mut self, response: Response<R>) {
        self.respond_impl(response)
    }

    fn respond_impl<R: Reader>(&mut self, response: Response<R>) {
        use std::io;

        fn passthrough<'a>(w: &'a mut Writer) -> &'a mut Writer { w }
        let mut writer = self.into_writer_impl();

        let do_not_send_body = self.method.equiv(&"HEAD");

        match response.raw_print(passthrough(writer),
                                self.http_version, self.headers.as_slice(),
                                do_not_send_body)
        {
            Ok(_) => (),
            Err(ref err) if err.kind == io::Closed => (),
            Err(ref err) if err.kind == io::BrokenPipe => (),
            Err(ref err) if err.kind == io::ConnectionAborted => (),
            Err(ref err) if err.kind == io::ConnectionRefused => (),
            Err(ref err) if err.kind == io::ConnectionReset => (),
            Err(ref err) =>
                println!("error while sending answer: {}", err)     // TODO: handle better?
        };

        writer.flush().ok();
    }
}

impl<'a> Iterator<Request> for IncomingRequests<'a> {
    fn next(&mut self) -> Option<Request> {
        self.server.recv().ok()
    }
}

impl std::fmt::Show for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter)
        -> Result<(), std::fmt::FormatError>
    {
        (format!("Request({} {} from {})",
            self.method, self.path, self.remote_addr.ip)).fmt(formatter)
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        if self.response_writer.is_some() {
            let response = Response::new_empty(StatusCode(500));
            self.respond_impl(response);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        use std::sync::atomics::Relaxed;
        self.close.store(true, Relaxed);
    }
}
