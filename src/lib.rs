/*!
# Simple usage

The first step is to create a `Server` object. To do so, simply call `Server::new()`.
The `new()` function returns an `IoResult<Server>` which will return an error
in the case where the server creation fails (for example if the listening port is already
occupied).

```rust
let server = httpd::Server::new().unwrap();
```

A newly-created `Server` will immediatly start listening for incoming connections and HTTP
requests.

Calling `server.recv()` will block until the next request is available.
This is usually what you should do if you write a website in Rust.

This function returns an `IoResult<Request>`, so you need to handle the possible errors.

```rust
loop {
    // blocks until the next request is received
    let request = match server.recv() {
        Ok(rq) => rq,
        Err(e) => { println!("error: {}", e); break }
    };

    // user-defined function to handle the request
    handle_request(request)
}
```

If you don't want to block, you can call `server.try_recv()` instead.

The `Request` object returned by `server.recv()` contains informations about the client's request.
The most useful methods are probably `request.get_method()` and `request.get_url()` which return
the requested method (GET, POST, etc.) and url.

To handle a request, you need to create a `Response` object. There are multiple
functions that allow you to create this object.
Here is an example of creating a Response from a file:

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
extern crate url;

use std::io::{Acceptor, IoError, IoResult, Listener};
use std::io::net::ip;
use std::io::net::tcp;
use std::sync;
use std::sync::Mutex;
use std::comm::Select;
use client::ClientConnection;

pub use common::{Header, HeaderField, HTTPVersion, Method, StatusCode};
pub use response::Response;

mod client;
mod common;
mod response;
mod sequential;
mod util;

/// The main class of this library.
/// 
/// Usually your code will look like this:
/// 
/// ```
/// let server = httpd::Server::new();
/// 
/// let pool = std::sync::TaskPool<()>::new(
///     std::cmp::min(1, std::os::num_cpus() - 1), || {}
/// );
///
/// loop {
///     let rq = match server.recv() {
///         Ok(rq) => rq,
///         Err(_) => break
///     };
///
///     pool.execute(proc(_) {
///         handle_request(rq)
///     });
/// }
/// ```
#[unstable]
pub struct Server {
    tasks_pool: Mutex<util::TaskPool>,
    connections_receiver: Receiver<IoResult<(ClientConnection, Sender<()>)>>,
    connections_close: Sender<()>,
    requests_receiver: sync::Mutex<Vec<(Receiver<Request>, Sender<()>)>>,
    listening_addr: ip::SocketAddr,
}

/// Represents an HTTP request made by a client.
///
/// A `Request` object is what is produced by the server, and is your what
///  your code must analyse and answer.
///
/// This object implements the `Send` trait, therefore you can spawn several threads to
///  handle multiple requests at once.
///
/// It is possible that multiple requests objects are linked to the same client, but
///  don't worry: the library automatically handles synchronization of the answers.
#[unstable]
pub struct Request {
    data_reader: Box<Reader + Send>,
    response_writer: Box<Writer + Send>,
    remote_addr: ip::SocketAddr,
    method: Method,
    path: url::Path,
    http_version: HTTPVersion,
    headers: Vec<Header>,
    body_length: uint,
}

enum ServerRecvEvent {
    NewRequest(Request),
    NewClient((ClientConnection, Sender<()>)),
    ReceiverErrord(uint),
    ServerSocketCrashed(IoError),
}

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
        // building the TcpAcceptor
        let mut listener = try!(tcp::TcpListener::bind(
            format!("{}", addr.ip).as_slice(), addr.port));
        let local_addr = try!(listener.socket_name());
        let server = try!(listener.listen());
        let (server, tx_close) = util::ClosableTcpAcceptor::new(server);

        // creating a task where server.accept() is continuously called
        // and ClientConnection objects are returned in the receiver
        let (tx_incoming, rx_incoming) = channel();

        spawn(proc() {
            let mut server = server;

            loop {
                let new_client = server.accept().map(|sock| {
                    use util::ClosableTcpStream;
                    let (read_closable, tx_close) = ClosableTcpStream::new(sock.clone(), true, false);
                    let (write_closable, _) = ClosableTcpStream::new(sock.clone(), false, true);
                    (ClientConnection::new(write_closable, read_closable), tx_close)
                });

                if tx_incoming.send_opt(new_client).is_err() {
                    break
                }
            }
        });

        // result
        Ok(Server {
            tasks_pool: Mutex::new(util::TaskPool::new()),
            connections_receiver: rx_incoming,
            connections_close: tx_close,
            requests_receiver: sync::Mutex::new(Vec::new()),
            listening_addr: local_addr,
        })
    }

    /// Returns the address the server is listening to.
    #[experimental]
    pub fn get_server_addr(&self) -> ip::SocketAddr {
        self.listening_addr
    }

    /// Returns the number of clients currently connected to the server.
    #[stable]
    pub fn get_num_connections(&self) -> uint {
        self.requests_receiver.lock().len()
    }

    /// Blocks until an HTTP request has been submitted and returns it.
    #[stable]
    pub fn recv(&self) -> IoResult<Request> {
        loop {
            match self.recv_impl() {
                NewClient(client) => self.add_client(client),
                NewRequest(rq) => return Ok(rq),
                ReceiverErrord(id) => { self.requests_receiver.lock().remove(id); },
                ServerSocketCrashed(err) => return Err(err)
            }
        }
    }

    /// Returns either a new client or a request, plus a list of connections that are
    ///   no longer valid
    fn recv_impl(&self) -> ServerRecvEvent {
        let mut locked_receivers = self.requests_receiver.lock();

        let select = Select::new();

        // add the handle for a new connection to the select
        let mut connections_handle = select.handle(&self.connections_receiver);
        unsafe { connections_handle.add() };

        // add all the existing connections
        let mut rq_handles = Vec::new();
        for rc in locked_receivers.iter() {
            rq_handles.push(select.handle(rc.ref0()));
        }
        for h in rq_handles.mut_iter() { unsafe { h.add() } }

        // getting the result
        loop {
            // yielding ; this function call is very important for good perfs
            { use std::task; task::deschedule(); }

            // waiting
            let handle_id = select.wait();

            // checking for connections_handle
            if handle_id == connections_handle.id() {
                match connections_handle.recv_opt() {
                    Ok(Ok(connec)) => {
                        for h in rq_handles.mut_iter() { unsafe { h.remove() } };
                        unsafe { connections_handle.remove() };
                        return NewClient(connec);
                    },
                    Ok(Err(err)) => {
                        for h in rq_handles.mut_iter() { unsafe { h.remove() } };
                        unsafe { connections_handle.remove() };
                        return ServerSocketCrashed(err)
                    },
                    _ => ()
                }
            }

            // checking the clients
            let mut result = None;
            for (id, h) in rq_handles.mut_iter().enumerate() {
                if handle_id == h.id() {
                    match h.recv_opt() {
                        Ok(rq) => result = Some(NewRequest(rq)),
                        Err(_) => result = Some(ReceiverErrord(id))
                    }
                    break
                }
            };

            match result {
                None => continue,
                Some(r) => {
                    for h in rq_handles.mut_iter() { unsafe { h.remove() } };
                    unsafe { connections_handle.remove() };
                    return r;
                }
            }
        }
    }

    /// Same as `recv()` but doesn't block.
    #[stable]
    pub fn try_recv(&self) -> IoResult<Option<Request>> {
        self.process_new_clients();

        {
            let mut locked_receivers = self.requests_receiver.lock();
            for rx in locked_receivers.iter() {
                let attempt = rx.ref0().try_recv();
                if attempt.is_ok() {       // TODO: remove the channel if it is closed
                    return Ok(Some(attempt.unwrap()));
                }
            }
        }

        Ok(None)
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
            self.add_client(client)
        }
    }

    /// Adds a new client to the list.
    fn add_client(&self, client: (ClientConnection, Sender<()>)) {
        let (client, tx_close) = client;

        let (tx, rx) = channel();

        {
            let mut locked_tasks_pool = self.tasks_pool.lock();
            locked_tasks_pool.spawn(proc() {
                let mut client = client;
                // TODO: when the channel is being closed, immediatly notify the task
                client.advance(|rq| tx.send_opt(rq).is_ok());
            });
        }

        self.requests_receiver.lock().push((rx, tx_close));
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

    /// Returns the length of the body in bytes.
    #[unstable]
    pub fn get_body_length(&self) -> uint {
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
    pub fn into_writer(self) -> Box<Writer + Send> {
        self.response_writer
    }

    /// Sends a response to this request.
    #[unstable]
    pub fn respond<R: Reader>(mut self, response: Response<R>) {
        fn passthrough<'a>(w: &'a mut Writer) -> &'a mut Writer { w }

        // TODO: pass a NullWriter if method is HEAD

        match response.raw_print(passthrough(self.response_writer),
                                self.http_version, self.headers.as_slice())
        {
            Ok(_) => (),
            Err(err) => println!("error while sending answer: {}", err)     // TODO: handle better?
        }
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

impl Drop for Server {
    fn drop(&mut self) {
        self.connections_close.send_opt(()).ok();
    }
}
