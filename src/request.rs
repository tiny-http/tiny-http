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

use ascii::AsciiCast;
use std::ascii::AsciiExt;

use std::io::Error as IoError;
use std::io::{self, Read, Write, ErrorKind};

use std::net::SocketAddr;
use std::fmt;
use std::str::FromStr;

use {Header, HTTPVersion, Method, Response, StatusCode};
use util::{AnyReader, AnyWriter};

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
    data_reader: Option<AnyReader>,

    // if this writer is empty, then the request has been answered
    response_writer: Option<AnyWriter>,

    remote_addr: SocketAddr,

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
    ExpectationFailed,
    CreationIoError(IoError),
}

/// Builds a new request
pub fn new_request<R, W>(method: Method, path: String,
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
        match headers.iter().find(|h: &&Header| h.field.equiv(&"Expect")) {
            None => false,
            Some(h) if h.value.eq_ignore_ascii_case(b"100-continue".to_ascii().unwrap())
                => true,
            _ => return Err(RequestCreationError::ExpectationFailed)
        }
    };

    // true if the client sent a `Connection: upgrade` header
    let connection_upgrade = {
        match headers.iter().find(|h: &&Header| h.field.equiv(&"Connection")) {
            None => false,
            Some(h) if h.value.eq_ignore_ascii_case(b"upgrade".to_ascii().unwrap())
                => true,
            _ => false
        }
    };

    // building the reader depending on
    //  transfer-encoding and content-length
    let reader =
        if connection_upgrade {
            // if we have a `Connection: upgrade`, always keeping the whole reader
            Box::new(source_data) as Box<Read + Send + 'static>

        } else if let Some(content_length) = content_length {
            if content_length == 0 {
                use std::io;
                Box::new(io::empty()) as Box<Read + Send + 'static>

            } else if content_length <= 1024 && !expects_continue {
                use std::io::Cursor;

                let mut buffer = vec![0; content_length];
                let mut offset = 0;

                loop {
                    if offset == content_length {
                        break;
                    }

                    let read = try!(source_data.read(&mut buffer[offset..])
                                               .map_err(|e| RequestCreationError::CreationIoError(e)));
                    if read == 0 {
                        break;
                    }

                    offset += read;
                }

                Box::new(Cursor::new(buffer)) as Box<Read + Send + 'static>

            } else {
                use util::EqualReader;
                let (data_reader, _) = EqualReader::new(source_data, content_length);   // TODO:
                Box::new(data_reader) as Box<Read + Send + 'static>
            }

        } else if transfer_encoding.is_some() {
            // if a transfer-encoding was specified, then "chunked"
            //  is ALWAYS applied over the message (RFC2616 #3.6)
            use util::ChunksDecoder;
            Box::new(ChunksDecoder::new(source_data)) as Box<Read + Send + 'static>

        } else {
            // if we have neither a Content-Length nor a Transfer-Encoding,
            // assuming that we have no data
            // TODO: could also be multipart/byteranges
            Box::new(io::empty()) as Box<Read + Send + 'static>
        };

    Ok(Request {
        data_reader: Some(AnyReader::new(reader)),
        response_writer: Some(AnyWriter::new(Box::new(writer) as Box<Write + Send + 'static>)),
        remote_addr: remote_addr,
        method: method,
        path: path,
        http_version: version,
        headers: headers,
        body_length: content_length,
        must_send_continue: expects_continue,
    })
}

impl Request {
    /// Returns the method requested by the client (eg. `GET`, `POST`, etc.).
    #[inline]
    pub fn get_method(&self) -> &Method {
        &self.method
    }

    /// Returns the resource requested by the client.
    #[inline]
    pub fn get_url(&self) -> &str {
        &self.path
    }

    /// Returns a list of all headers sent by the client.
    #[inline]
    pub fn get_headers(&self) -> &[Header] {
        &self.headers
    }

    /// Returns the HTTP version of the request.
    #[inline]
    pub fn get_http_version(&self) -> &HTTPVersion {
        &self.http_version
    }

    /// Returns the length of the body in bytes.
    ///
    /// Returns `None` if the length is unknown.
    #[inline]
    pub fn get_body_length(&self) -> Option<usize> {
        self.body_length
    }

    /// Returns the length of the body in bytes.
    #[inline]
    pub fn get_remote_addr(&self) -> &SocketAddr {
        &self.remote_addr
    }

/*      // FIXME: reimplement this
    /// Sends a response with a `Connection: upgrade` header, then turns the `Request` into a `Stream`.
    ///
    /// The main purpose of this function is to support websockets.
    /// If you detect that the request wants to use some kind of protocol upgrade, you can
    ///  call this function to obtain full control of the socket stream.
    ///
    /// If you call this on a non-websocket request, tiny-http will wait until this `Stream` object
    ///  is destroyed before continuing to read or write on the socket. Therefore you should always
    ///  destroy it as soon as possible.
    pub fn upgrade<R: Read>(mut self, protocol: &str, response: Response<R>) -> Box<Read + Write + Send> {
        use util::CustomStream;

        response.raw_print(self.response_writer.as_mut().unwrap().by_ref(), self.http_version,
                           self.headers, false, Some(protocol)).ok();   // TODO: unused result

        self.response_writer.as_mut().unwrap().flush().ok();    // TODO: unused result

        let stream = CustomStream::new(self.into_reader_impl(), self.into_writer_impl());
        Box::new(stream) as Box<Read + Write + Send>
    }*/

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
    /// # let server = tiny_http::ServerBuilder::new().build().unwrap();
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

        fn passthrough<'a>(r: &'a mut Read) -> &'a mut Read { r }
        passthrough(self.data_reader.as_mut().unwrap())
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
        self.into_writer_impl().unwrap()
    }

    fn into_writer_impl(&mut self) -> AnyWriter {
        use std::mem;

        assert!(self.response_writer.is_some());

        let mut writer = None;
        mem::swap(&mut self.response_writer, &mut writer);
        writer.unwrap()
    }

    fn into_reader_impl(&mut self) -> AnyReader {
        use std::mem;

        assert!(self.data_reader.is_some());

        let mut reader = None;
        mem::swap(&mut self.data_reader, &mut reader);
        reader.unwrap()
    }

    /// Sends a response to this request.
    #[inline]
    pub fn respond<R>(mut self, response: Response<R>) where R: Read {
        self.respond_impl(response)
    }

    fn respond_impl<R>(&mut self, response: Response<R>) where R: Read {
        let mut writer = self.into_writer_impl();

        let do_not_send_body = self.method.equiv(&"HEAD");

        match response.raw_print(writer.by_ref(),
                                 self.http_version.clone(), &self.headers,
                                 do_not_send_body, None)
        {
            Ok(_) => (),
            Err(ref err) if err.kind() == ErrorKind::BrokenPipe => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionAborted => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionRefused => (),
            Err(ref err) if err.kind() == ErrorKind::ConnectionReset => (),
            Err(ref err) =>
                println!("error while sending answer: {}", err)     // TODO: handle better?
        };

        writer.flush().ok();
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "Request({} {} from {})", self.method, self.path, self.remote_addr)
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

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn must_be_send() {
        fn f<T: Send>(_: &T) {}
        fn bar(rq: &Request) { f(rq); }
    }
}
