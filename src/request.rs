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

use std::ascii::AsciiExt;

use std::io::Error as IoError;
use std::io::{self, Cursor, Read, Write, ErrorKind};

use std::net::SocketAddr;
use std::fmt;
use std::str::FromStr;

use {Header, HTTPVersion, Method, Response, StatusCode};
use util::EqualReader;
use chunked_transfer::Decoder;

/// Represents an HTTP request made by a client.
///
/// A `Request` object is what is produced by the server, and is your what
/// your code must analyse and answer.
///
/// This object implements the `Send` trait, therefore you can dispatch your requests to
/// worker threads.
///
/// # Pipelining
///
/// If a client sends multiple requests in a row (without waiting for the response), then you will
/// get multiple `Request` objects simultaneously. This is called *requests pipelining*.
/// Tiny-http automatically reorders the responses so that you don't need to worry about the order
/// in which you call `respond` or `into_writer`.
///
/// This mechanic is disabled if:
///
///  - The body of a request is large enough (handling requires pipelining requires storing the
///    body of the request in a buffer ; if the body is too big, tiny-http will avoid doing that)
///  - A request sends a `Expect: 100-continue` header (which means that the client waits to
///    know whether its body will be processed before sending it)
///  - A request sends a `Connection: close` header or `Connection: upgrade` header (used for
///    websockets), which indicates that this is the last request that will be received on this
///    connection
///
/// # Automatic cleanup
///
/// If a `Request` object is destroyed without `into_writer` or `respond` being called,
/// an empty response with a 500 status code (internal server error) will automatically be
/// sent back to the client.
/// This means that if your code fails during the handling of a request, this "internal server
/// error" response will automatically be sent during the stack unwinding.
pub struct Request {
    // where to read the body from
    data_reader: Option<Box<Read + Send + 'static>>,

    // if this writer is empty, then the request has been answered
    response_writer: Option<Box<Write + Send + 'static>>,

    remote_addr: SocketAddr,

    // true if HTTPS, false if HTTP
    secure: bool,

    method: Method,

    path: String,

    http_version: HTTPVersion,

    headers: Vec<Header>,

    body_length: Option<usize>,

    // true if a `100 Continue` response must be sent when `as_reader()` is called
    must_send_continue: bool,
}

/// Error that can happen when building a `Request` object.
pub enum RequestCreationError {
    /// The client sent an `Expect` header that was not recognized by tiny-http.
    ExpectationFailed,

    /// Error while reading data from the socket during the creation of the `Request`.
    CreationIoError(IoError),
}

impl From<IoError> for RequestCreationError {
    fn from(err: IoError) -> RequestCreationError {
        RequestCreationError::CreationIoError(err)
    }
}

/// Builds a new request.
///
/// After the request line and headers have been read from the socket, a new `Request` object
/// is built.
///
/// You must pass a `Read` that will allow the `Request` object to read from the incoming data.
/// It is the responsibility of the `Request` to read only the data of the request and not further.
///
/// The `Write` object will be used by the `Request` to write the response.
pub fn new_request<R, W>(secure: bool, method: Method, path: String,
                         version: HTTPVersion, headers: Vec<Header>,
                         remote_addr: SocketAddr, mut source_data: R, writer: W)
                         -> Result<Request, RequestCreationError>
                         where R: Read + Send + 'static, W: Write + Send + 'static
{
    // finding the transfer-encoding header
    let transfer_encoding = headers.iter()
        .find(|h: &&Header| h.field.equiv(&"Transfer-Encoding"))
        .map(|h| h.value.clone());

    // finding the content-length header
    let content_length = if transfer_encoding.is_some() {
        // if transfer-encoding is specified, the Content-Length
        // header must be ignored (RFC2616 #4.4)
        None

    } else {
        headers.iter()
               .find(|h: &&Header| h.field.equiv(&"Content-Length"))
               .and_then(|h| FromStr::from_str(h.value.as_str()).ok())
    };

    // true if the client sent a `Expect: 100-continue` header
    let expects_continue = {
        match headers.iter().find(|h: &&Header| h.field.equiv(&"Expect")).map(|h| AsRef::<str>::as_ref(h.value.as_ref())) {
            None => false,
            Some(v) if v.eq_ignore_ascii_case("100-continue")
                => true,
            _ => return Err(RequestCreationError::ExpectationFailed)
        }
    };

    // true if the client sent a `Connection: upgrade` header
    let connection_upgrade = {
        match headers.iter().find(|h: &&Header| h.field.equiv(&"Connection")).map(|h| AsRef::<str>::as_ref(h.value.as_ref())) {
            Some(v) if v.to_ascii_lowercase().contains("upgrade")
                => true,
            _ => false
        }
    };

    // we wrap `source_data` around a reading whose nature depends on the transfer-encoding and
    // content-length headers
    let reader =
        if connection_upgrade {
            // if we have a `Connection: upgrade`, always keeping the whole reader
            Box::new(source_data) as Box<Read + Send + 'static>

        } else if let Some(content_length) = content_length {
            if content_length == 0 {
                Box::new(io::empty()) as Box<Read + Send + 'static>

            } else if content_length <= 1024 && !expects_continue {
                // if the content-length is small enough, we just read everything into a buffer

                let mut buffer = vec![0; content_length];
                let mut offset = 0;

                while offset != content_length {
                    let read = try!(source_data.read(&mut buffer[offset..]));
                    if read == 0 {
                        // the socket returned EOF, but we were before the expected content-length
                        // aborting
                        let info = "Connection has been closed before we received enough data";
                        let err = IoError::new(ErrorKind::ConnectionAborted, info);
                        return Err(RequestCreationError::CreationIoError(err));
                    }

                    offset += read;
                }

                Box::new(Cursor::new(buffer)) as Box<Read + Send + 'static>

            } else {
                let (data_reader, _) = EqualReader::new(source_data, content_length);   // TODO:
                Box::new(data_reader) as Box<Read + Send + 'static>
            }

        } else if transfer_encoding.is_some() {
            // if a transfer-encoding was specified, then "chunked" is ALWAYS applied
            // over the message (RFC2616 #3.6)
            Box::new(Decoder::new(source_data)) as Box<Read + Send + 'static>

        } else {
            // if we have neither a Content-Length nor a Transfer-Encoding,
            // assuming that we have no data
            // TODO: could also be multipart/byteranges
            Box::new(io::empty()) as Box<Read + Send + 'static>
        };

    Ok(Request {
        data_reader: Some(reader),
        response_writer: Some(Box::new(writer) as Box<Write + Send + 'static>),
        remote_addr: remote_addr,
        secure: secure,
        method: method,
        path: path,
        http_version: version,
        headers: headers,
        body_length: content_length,
        must_send_continue: expects_continue,
    })
}

impl Request {
    /// Returns true if the request was made through HTTPS.
    #[inline]
    pub fn secure(&self) -> bool {
        self.secure
    }

    /// Returns the method requested by the client (eg. `GET`, `POST`, etc.).
    #[inline]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Returns the resource requested by the client.
    #[inline]
    pub fn url(&self) -> &str {
        &self.path
    }

    /// Returns a list of all headers sent by the client.
    #[inline]
    pub fn headers(&self) -> &[Header] {
        &self.headers
    }

    /// Returns the HTTP version of the request.
    #[inline]
    pub fn http_version(&self) -> &HTTPVersion {
        &self.http_version
    }

    /// Returns the length of the body in bytes.
    ///
    /// Returns `None` if the length is unknown.
    #[inline]
    pub fn body_length(&self) -> Option<usize> {
        self.body_length
    }

    /// Returns the address of the client that sent this request.
    ///
    /// Note that this is gathered from the socket. If you receive the request from a proxy,
    /// this function will return the address of the proxy and not the address of the actual
    /// user.
    #[inline]
    pub fn remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }

    /// Sends a response with a `Connection: upgrade` header, then turns the `Request` into a `Stream`.
    ///
    /// The main purpose of this function is to support websockets.
    /// If you detect that the request wants to use some kind of protocol upgrade, you can
    ///  call this function to obtain full control of the socket stream.
    ///
    /// If you call this on a non-websocket request, tiny-http will wait until this `Stream` object
    ///  is destroyed before continuing to read or write on the socket. Therefore you should always
    ///  destroy it as soon as possible.
    pub fn upgrade<R: Read>(mut self, protocol: &str, response: Response<R>) -> Box<ReadWrite + Send> {
        use util::CustomStream;

        response.raw_print(self.response_writer.as_mut().unwrap().by_ref(), self.http_version.clone(),
                           &self.headers, false, Some(protocol)).ok();   // TODO: unused result

        self.response_writer.as_mut().unwrap().flush().ok();    // TODO: unused result

        let stream = CustomStream::new(self.into_reader_impl(), self.into_writer_impl());
        Box::new(stream) as Box<ReadWrite + Send>
    }

    /// Allows to read the body of the request.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # extern crate rustc_serialize;
    /// # extern crate tiny_http;
    /// # use rustc_serialize::json::Json;
    /// # use std::io::Read;
    /// # fn get_content_type(_: &tiny_http::Request) -> &'static str { "" }
    /// # fn main() {
    /// # let server = tiny_http::Server::http("0.0.0.0:0").unwrap();
    /// let mut request = server.recv().unwrap();
    ///
    /// if get_content_type(&request) == "application/json" {
    ///     let mut content = String::new();
    ///     request.as_reader().read_to_string(&mut content).unwrap();
    ///     let json: Json = content.parse().unwrap();
    /// }
    /// # }
    /// ```
    ///
    /// If the client sent a `Expect: 100-continue` header with the request, calling this
    ///  function will send back a `100 Continue` response.
    #[inline]
    pub fn as_reader(&mut self) -> &mut Read {
        if self.must_send_continue {
            let msg = Response::new_empty(StatusCode(100));
            msg.raw_print(self.response_writer.as_mut().unwrap().by_ref(),
                          self.http_version.clone(), &self.headers, true, None).ok();
            self.response_writer.as_mut().unwrap().flush().ok();
            self.must_send_continue = false;
        }

        self.data_reader.as_mut().unwrap()
    }

    /// Turns the `Request` into a writer.
    ///
    /// The writer has a raw access to the stream to the user.
    /// This function is useful for things like CGI.
    ///
    /// Note that the destruction of the `Writer` object may trigger
    /// some events. For exemple if a client has sent multiple requests and the requests
    /// have been processed in parallel, the destruction of a writer will trigger
    /// the writing of the next response.
    /// Therefore you should always destroy the `Writer` as soon as possible.
    #[inline]
    pub fn into_writer(mut self) -> Box<Write + Send + 'static> {
        self.into_writer_impl()
    }

    fn into_writer_impl(&mut self) -> Box<Write + Send + 'static> {
        use std::mem;

        assert!(self.response_writer.is_some());

        let mut writer = None;
        mem::swap(&mut self.response_writer, &mut writer);
        writer.unwrap()
    }

    fn into_reader_impl(&mut self) -> Box<Read + Send + 'static> {
        use std::mem;

        assert!(self.data_reader.is_some());

        let mut reader = None;
        mem::swap(&mut self.data_reader, &mut reader);
        reader.unwrap()
    }

    /// Sends a response to this request.
    #[inline]
    pub fn respond<R>(mut self, response: Response<R>) -> Result<(), IoError>
        where R: Read
    {
        self.respond_impl(response)
    }

    fn respond_impl<R>(&mut self, response: Response<R>) -> Result<(), IoError>
        where R: Read
    {
        // Droping the request reader now so that further requests can start processing immediately.
        self.data_reader = None;

        let mut writer = self.into_writer_impl();

        let do_not_send_body = self.method == Method::Head;

        match response.raw_print(writer.by_ref(),
                                 self.http_version.clone(), &self.headers,
                                 do_not_send_body, None)
        {
            Ok(_) => (),
            Err(ref err) if err.kind() == ErrorKind::BrokenPipe => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionAborted => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionRefused => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionReset => (),
            Err(err) => return Err(err)
        };

        writer.flush()
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "Request({} {} from {})", self.method, self.path, self.remote_addr)
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // Droping the request reader now so that further requests can start processing immediately.
        self.data_reader = None;

        if self.response_writer.is_some() {
            let response = Response::empty(500);
            let _ = self.respond_impl(response);        // ignoring any potential error
        }
    }
}

/// Dummy trait that regroups the `Read` and `Write` traits.
///
/// Automatically implemented on all types that implement both `Read` and `Write`.
pub trait ReadWrite: Read + Write {}
impl<T> ReadWrite for T where T: Read + Write {}

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn must_be_send() {
        #![allow(dead_code)]
        fn f<T: Send>(_: &T) {}
        fn bar(rq: &Request) { f(rq); }
    }
}
