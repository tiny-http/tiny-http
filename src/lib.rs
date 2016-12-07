// Copyright 2015 The tiny-http Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/*!
# Simple usage

## Creating the server

The easiest way to create a server is to call `Server::http()`.

The `http()` function returns an `IoResult<Server>` which will return an error
in the case where the server creation fails (for example if the listening port is already
occupied).

```no_run
let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
```

A newly-created `Server` will immediatly start listening for incoming connections and HTTP
requests.

## Receiving requests

Calling `server.recv()` will block until the next request is available.
This function returns an `IoResult<Request>`, so you need to handle the possible errors.

```no_run
# let server = tiny_http::Server::http("0.0.0.0:0").unwrap();

loop {
    // blocks until the next request is received
    let request = match server.recv() {
        Ok(rq) => rq,
        Err(e) => { println!("error: {}", e); break }
    };

    // do something with the request
    // ...
}
```

In a real-case scenario, you will probably want to spawn multiple worker tasks and call
`server.recv()` on all of them. Like this:

```no_run
# use std::sync::Arc;
# use std::thread;
# let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
let server = Arc::new(server);
let mut guards = Vec::with_capacity(4);

for _ in (0 .. 4) {
    let server = server.clone();

    let guard = thread::spawn(move || {
        loop {
            let rq = server.recv().unwrap();

            // ...
        }
    });

    guards.push(guard);
}
```

If you don't want to block, you can call `server.try_recv()` instead.

## Handling requests

The `Request` object returned by `server.recv()` contains informations about the client's request.
The most useful methods are probably `request.method()` and `request.url()` which return
the requested method (`GET`, `POST`, etc.) and url.

To handle a request, you need to create a `Response` object. See the docs of this object for
more infos. Here is an example of creating a `Response` from a file:

```no_run
# use std::fs::File;
# use std::path::Path;
let response = tiny_http::Response::from_file(File::open(&Path::new("image.png")).unwrap());
```

All that remains to do is call `request.respond()`:

```no_run
# use std::fs::File;
# use std::path::Path;
# let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
# let request = server.recv().unwrap();
# let response = tiny_http::Response::from_file(File::open(&Path::new("image.png")).unwrap());
let _ = request.respond(response);
```
*/
#![crate_name = "tiny_http"]
#![crate_type = "lib"]
#![forbid(unsafe_code)]

#[macro_use]
extern crate log;

extern crate ascii;
extern crate chunked_transfer;
extern crate encoding;
extern crate url;
extern crate chrono;

#[cfg(feature = "ssl")]
extern crate openssl;

use std::error::Error;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::net;
use std::net::{ToSocketAddrs, TcpStream, Shutdown};
use std::time::Duration;
use std::sync::atomic::Ordering::Relaxed;

use client::ClientConnection;
use util::MessagesQueue;

pub use common::{Header, HeaderField, HTTPVersion, Method, StatusCode};
pub use request::{Request, ReadWrite};
pub use response::{ResponseBox, Response};

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
pub struct Server {
    // should be false as long as the server exists
    // when set to true, all the subtasks will close within a few hundreds ms
    close: Arc<AtomicBool>,

    // queue for messages received by child threads
    messages: Arc<MessagesQueue<Message>>,

    // result of TcpListener::local_addr()
    listening_addr: net::SocketAddr,
}

enum Message {
    Error(IoError),
    NewRequest(Request),
}

impl From<IoError> for Message {
    fn from(e: IoError) -> Message {
        Message::Error(e)
    }
}

impl From<Request> for Message {
    fn from(rq: Request) -> Message {
        Message::NewRequest(rq)
    }
}

// this trait is to make sure that Server implements Share and Send
#[doc(hidden)]
trait MustBeShareDummy : Sync + Send {}
#[doc(hidden)]
impl MustBeShareDummy for Server {}


pub struct IncomingRequests<'a> {
    server: &'a Server
}

/// Represents the parameters required to create a server.
#[derive(Debug, Clone)]
pub struct ServerConfig<A> where A: ToSocketAddrs {
    /// The addresses to listen to.
    pub addr: A,

    /// If `Some`, then the server will use SSL to encode the communications.
    pub ssl: Option<SslConfig>,
}

/// Configuration of the server for SSL.
#[derive(Debug, Clone)]
pub struct SslConfig {
    /// Contains the public certificate to send to clients.
    pub certificate: Vec<u8>,
    /// Contains the ultra-secret private key used to decode communications.
    pub private_key: Vec<u8>,
}

impl Server {
    /// Shortcut for a simple server on a specific address.
    #[inline]
    pub fn http<A>(addr: A) -> Result<Server, Box<Error + Send + Sync + 'static>>
        where A: ToSocketAddrs
    {
        Server::new(ServerConfig {
            addr: addr,
            ssl: None,
        })
    }

    /// Shortcut for an HTTPS server on a specific address.
    #[cfg(feature = "ssl")]
    #[inline]
    pub fn https<A>(addr: A, config: SslConfig)
                    -> Result<Server, Box<Error + Send + Sync + 'static>>
        where A: ToSocketAddrs
    {
        Server::new(ServerConfig {
            addr: addr,
            ssl: Some(config),
        })
    }

    /// Builds a new server that listens on the specified address.
    pub fn new<A>(config: ServerConfig<A>) -> Result<Server, Box<Error + Send + Sync + 'static>>
        where A: ToSocketAddrs
    {
        // building the "close" variable
        let close_trigger = Arc::new(AtomicBool::new(false));

        // building the TcpListener
        let (server, local_addr) = {
            let listener = try!(net::TcpListener::bind(config.addr));
            let local_addr = try!(listener.local_addr());
            debug!("Server listening on {}", local_addr);
            (listener, local_addr)
        };

        // building the SSL capabilities
        #[cfg(feature = "ssl")]
        type SslContext = openssl::ssl::SslContext;
        #[cfg(not(feature = "ssl"))]
        type SslContext = ();
        let ssl: Option<SslContext> = match config.ssl {
            #[cfg(feature = "ssl")]
            Some(mut config) => {
                use std::io::Cursor;
                use openssl::ssl;
                use openssl::x509::X509;
                use openssl::crypto::pkey::PKey;
                use openssl::ssl::SSL_VERIFY_NONE;

                let mut ctxt = try!(SslContext::new(ssl::SslMethod::Sslv23));
                try!(ctxt.set_cipher_list("DEFAULT"));
                let certificate = try!(X509::from_pem(&mut Cursor::new(&config.certificate[..])));
                try!(ctxt.set_certificate(&certificate));
                let private_key = try!(PKey::private_key_from_pem(&mut Cursor::new(&config.private_key[..])));
                try!(ctxt.set_private_key(&private_key));
                ctxt.set_verify(SSL_VERIFY_NONE, None);
                try!(ctxt.check_private_key());

                // let's wipe the certificate and private key from memory, because we're
                // better safe than sorry
                for b in &mut config.certificate { *b = 0; }
                for b in &mut config.private_key { *b = 0; }

                Some(ctxt)
            },
            #[cfg(not(feature = "ssl"))]
            Some(_) => return Err("Building a server with SSL requires enabling the `ssl` feature \
                                   in tiny-http".to_owned().into()),
            None => None,
        };

        // creating a task where server.accept() is continuously called
        // and ClientConnection objects are pushed in the messages queue
        let messages = MessagesQueue::with_capacity(8);

        let inside_close_trigger = close_trigger.clone();
        let inside_messages = messages.clone();
        thread::spawn(move || {
            // a tasks pool is used to dispatch the connections into threads
            let tasks_pool = util::TaskPool::new();

            debug!("Running accept thread");
            while !inside_close_trigger.load(Relaxed) {
                let new_client = match server.accept() {
                    Ok((sock, _)) => {
                        use util::RefinedTcpStream;
                        let (read_closable, write_closable) = match ssl {
                            None => {
                                RefinedTcpStream::new(sock)
                            },
                            #[cfg(feature = "ssl")]
                            Some(ref ssl) => {
                                // trying to apply SSL over the connection
                                // if an error occurs, we just close the socket and resume listening
                                let sock = match openssl::ssl::SslStream::accept(ssl, sock) {
                                    Ok(s) => s,
                                    Err(_) => continue
                                };

                                RefinedTcpStream::new(sock)
                            },
                            #[cfg(not(feature = "ssl"))]
                            Some(_) => unreachable!(),
                        };

                        Ok(ClientConnection::new(write_closable, read_closable))
                    },
                    Err(e) => Err(e),
                };

                match new_client {
                    Ok(client) => {
                        let messages = inside_messages.clone();
                        let mut client = Some(client);
                        tasks_pool.spawn(Box::new(move || {
                            if let Some(client) = client.take() {
                                for rq in client {
                                    messages.push(rq.into());
                                }
                            }
                        }));
                    },

                    Err(e) => {
                        error!("Error accepting new client: {}", e);
                        inside_messages.push(e.into());
                        break;
                    }
                }
            }
            debug!("Terminating accept thread");
        });

        // result
        Ok(Server {
            messages: messages,
            close: close_trigger,
            listening_addr: local_addr,
        })
    }

    /// Returns an iterator for all the incoming requests.
    ///
    /// The iterator will return `None` if the server socket is shutdown.
    #[inline]
    pub fn incoming_requests(&self) -> IncomingRequests {
        IncomingRequests { server: self }
    }

    /// Returns the address the server is listening to.
    #[inline]
    pub fn server_addr(&self) -> net::SocketAddr {
        self.listening_addr.clone()
    }

    /// Returns the number of clients currently connected to the server.
    pub fn num_connections(&self) -> usize {
        unimplemented!()
        //self.requests_receiver.lock().len()
    }

    /// Blocks until an HTTP request has been submitted and returns it.
    pub fn recv(&self) -> IoResult<Request> {
        match self.messages.pop() {
            Message::Error(err) => return Err(err),
            Message::NewRequest(rq) => return Ok(rq),
        }
    }

    /// Same as `recv()` but doesn't block longer than timeout
    pub fn recv_timeout(&self, timeout: Duration) -> IoResult<Option<Request>> {
        match self.messages.pop_timeout(timeout) {
            Some(Message::Error(err)) => return Err(err),
            Some(Message::NewRequest(rq)) => return Ok(Some(rq)),
            None => return Ok(None)
        }
    }

    /// Same as `recv()` but doesn't block.
    pub fn try_recv(&self) -> IoResult<Option<Request>> {
        match self.messages.try_pop() {
            Some(Message::Error(err)) => return Err(err),
            Some(Message::NewRequest(rq)) => return Ok(Some(rq)),
            None => return Ok(None)
        }
    }
}

impl<'a> Iterator for IncomingRequests<'a> {
    type Item = Request;
    fn next(&mut self) -> Option<Request> {
        self.server.recv().ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.close.store(true, Relaxed);
        // Connect briefly to ourselves to unblock the accept thread
        let maybe_stream = TcpStream::connect(self.listening_addr);
        if let Ok(stream) = maybe_stream {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}
