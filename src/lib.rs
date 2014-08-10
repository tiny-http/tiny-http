/*!
# Simple usage

## Creating the server

The easiest way to create a server is to call `Server::new()`.

The `new()` function returns an `IoResult<Server>` which will return an error
in the case where the server creation fails (for example if the listening port is already
occupied).

```rust
let server = httpd::ServerBuilder::new().build().unwrap();
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
    let server = server.clone();

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
extern crate flate;
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
pub use request::Request;
pub use response::Response;

mod client;
mod common;
mod request;
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
#[doc(hidden)]
trait MustBeShareDummy : Sync + Send {}
#[doc(hidden)]
impl MustBeShareDummy for Server {}

#[unstable]
pub struct IncomingRequests<'a> {
    server: &'a Server
}

/// Object which allows you to build a server.
pub struct ServerBuilder {
    // the address to listen to
    address: ip::SocketAddr,

    // number of milliseconds before client timeout
    client_timeout_ms: u32,

    // maximum number of clients before 503
    // TODO: 
    //max_clients: uint,
}

impl ServerBuilder {
    /// Creates a new builder.
    pub fn new() -> ServerBuilder {
        ServerBuilder {
            address: ip::SocketAddr { ip: ip::Ipv4Addr(0, 0, 0, 0), port: 80 },
            client_timeout_ms: 60 * 1000,
            //max_clients: { use std::num::Bounded; Bounded::max_value() },
        }
    }

    /// The server will use a precise port.
    pub fn with_port(mut self, port: ip::Port) -> ServerBuilder {
        self.address.port = port;
        self
    }

    /// The server will use a random port.
    ///
    /// Call `server.get_server_addr()` to retreive it once the server is created.
    pub fn with_random_port(mut self) -> ServerBuilder {
        self.address.port = 0;
        self
    }

    /// The server will use a precise port.
    pub fn with_client_connections_timeout(mut self, milliseconds: u32) -> ServerBuilder {
        self.client_timeout_ms = milliseconds;
        self
    }

    /// Builds the server with the given configuration.
    pub fn build(self) -> IoResult<Server> {
        Server::new(self)
    }
}

impl Server {
    /// Builds a new server that listens on the specified address.
    fn new(config: ServerBuilder) -> IoResult<Server> {
        // building the "close" variable
        let close_trigger = Arc::new(AtomicBool::new(false));

        // building the TcpAcceptor
        let (server, local_addr) = {
            let mut listener = try!(tcp::TcpListener::bind(
                format!("{}", config.address.ip).as_slice(), config.address.port));
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
                        inside_close_trigger.clone(), true, false,
                        config.client_timeout_ms);

                    let write_closable = ClosableTcpStream::new(sock.clone(),
                        inside_close_trigger.clone(), false, true,
                        config.client_timeout_ms);

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
    #[inline]
    pub fn incoming_requests<'a>(&'a self) -> IncomingRequests<'a> {
        IncomingRequests { server: self }
    }

    /// Returns the address the server is listening to.
    #[experimental]
    #[inline]
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

            for rq in client {
                let res = requests_sender.send_opt(rq);
                if res.is_err() {
                    break
                }
            }
        });
    }
}

impl<'a> Iterator<Request> for IncomingRequests<'a> {
    fn next(&mut self) -> Option<Request> {
        self.server.recv().ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        use std::sync::atomics::Relaxed;
        self.close.store(true, Relaxed);
    }
}
