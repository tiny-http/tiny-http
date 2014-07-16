## tiny-http

[![Build Status](https://travis-ci.org/tomaka/tiny-http.svg?branch=master)](https://travis-ci.org/tomaka/tiny-http)

Tiny but strong HTTP server in Rust.

What does **tiny-http** handle?
 - Accepting and managing connections to the clients
 - Requests pipelining
 - Parsing request headers
 - Transfer-Encoding and Content-Encoding (**not implemented yet**)
 - Turning user input (eg. POST input) into UTF-8 (**not implemented yet**)
 - Ranges (**not implemented yet**)
 - HTTPS (**not implemented yet**)

Tiny-http handles everything that is related to client connections and data transfers and encoding.

Everything else (multipart data, routing, etags, cache-control, HTML templates, etc.) must be handled by your code.
If you want to create a website in Rust, I strongly recommend using a framework instead of this library.

[**Link to the documentation**](http://www.rust-ci.org/tomaka/tiny-http/doc/tiny-http/index.html)

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

### [Usage](http://www.rust-ci.org/tomaka/tiny-http/doc/tiny-http/index.html)

Check the documentation!

### Some benchmarking

On my local machine, `ab -c 20 -n 1000 -k http://localhost/` gives:
 - ~0.65 sec for apache2
 - ~1.56 sec for nodejs
 - ~1.074 sec for rust-http
 - ~1.141 sec for tiny-http
