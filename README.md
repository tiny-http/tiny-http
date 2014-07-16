## tiny-http

[![Build Status](https://travis-ci.org/tomaka/tiny-http.svg?branch=master)](https://travis-ci.org/tomaka/tiny-http)

Tiny but strong HTTP server in Rust.

What does **tiny-http** handle?
 - Accepting and managing connections to the clients
 - Parsing request headers
 - Transfer-Encoding and Content-Encoding (**not implemented yet**)
 - Turning user input (eg. POST input) into UTF-8 (**not implemented yet**)
 - Ranges (**not implemented yet**)
 - HTTPS (**not implemented yet**)

Tiny-http handles everything that is related to client connections and data transfers and encoding.

Everything else (multipart data, routing, etags, cache-control, HTML templates, etc.) must be handled by your code.
If you want to create a website in Rust, I strongly recommend using a framework instead of this library.

[**Link to the documentation**](http://www.rust-ci.org/tomaka/tiny-http/doc/tiny-http/)

### Installation

Add this to the `Cargo.toml` file of your project:

```toml
[dependencies.tiny-http]
git = "https://github.com/tomaka/tiny-http"
```

Don't forget to add the external crate:

```rust
extern crate httpd = "tiny-http"
```

### Simple usage

The first step is to create a `Server` object. To do so, simply call `Server::new()`.
The `new()` function returns an `IoResult<Server>` which will return an error
in the case where the server creation fails (for example if the listening port is already occupied).

```rust
let server = httpd::Server::new().unwrap();
```

A newly-created `Server` will immediatly start listening for incoming connections and HTTP requests.

Calling `server.recv()` will block until the next request is available. This is usually what you should do
if you write a website in Rust.

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
The most useful methods are probably `request.get_method()` and `request.get_url()` which return the
requested method (GET, POST, etc.) and url.

To handle a request, you need to create a `Response` object. There are multiple functions that allow you
to create this object. Here is an example of creating a Response from a file:

```rust
let response = httpd::Response::from_file(Path::new("image.png"));
```

All that remains to do is call `request.respond()`:

```rust
request.respond(response)
```

### Some benchmarking

On my local machine, `ab -c 20 -n 1000 -k http://localhost/` gives:
 - ~0.65 sec for apache2
 - ~1.56 sec for nodejs
 - ~1.074 sec for rust-http
 - ~3.98 sec for tiny-http
