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

use ascii::{AsciiString};
use std::ascii::AsciiExt;

use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::io::{ErrorKind, Read, BufReader, BufWriter};

use std::net::SocketAddr;
use std::str::FromStr;

use common::{HTTPVersion, Method};
use util::{SequentialReader, SequentialReaderBuilder, SequentialWriterBuilder};
use util::RefinedTcpStream;

use Request;

/// A ClientConnection is an object that will store a socket to a client
/// and return Request objects.
pub struct ClientConnection {
    // address of the client
    remote_addr: IoResult<SocketAddr>,

    // sequence of Readers to the stream, so that the data is not read in
    //  the wrong order
    source: SequentialReaderBuilder<BufReader<RefinedTcpStream>>,

    // sequence of Writers to the stream, to avoid writing response #2 before
    //  response #1
    sink: SequentialWriterBuilder<BufWriter<RefinedTcpStream>>,

    // Reader to read the next header from
	next_header_source: SequentialReader<BufReader<RefinedTcpStream>>,

    // set to true if we know that the previous request is the last one
    no_more_requests: bool,

    // true if the connection goes through SSL
    secure: bool,
}

/// Error that can happen when reading a request.
enum ReadError {
    WrongRequestLine,
    WrongHeader(HTTPVersion),

    /// the client sent an unrecognized `Expect` header
    ExpectationFailed(HTTPVersion),

    ReadIoError(IoError),
}

impl ClientConnection {
    /// Creates a new ClientConnection that takes ownership of the TcpStream.
    pub fn new(write_socket: RefinedTcpStream, mut read_socket: RefinedTcpStream)
               -> ClientConnection
    {
        let remote_addr = read_socket.peer_addr();
        let secure = read_socket.secure();

        let mut source = SequentialReaderBuilder::new(BufReader::with_capacity(1024, read_socket));
        let first_header = source.next().unwrap();

        ClientConnection {
            source: source,
            sink: SequentialWriterBuilder::new(BufWriter::with_capacity(1024, write_socket)),
            remote_addr: remote_addr,
            next_header_source: first_header,
            no_more_requests: false,
            secure: secure,
        }
    }

    /// Reads the next line from self.next_header_source.
    ///
    /// Reads until `CRLF` is reached. The next read will start
    ///  at the first byte of the new line.
    fn read_next_line(&mut self) -> IoResult<AsciiString> {
        let mut buf = Vec::new();
        let mut prev_byte_was_cr = false;

        loop {
            let byte = self.next_header_source.by_ref().bytes().next();

            let byte = match byte {
                Some(b) => try!(b),
                None => return Err(IoError::new(ErrorKind::ConnectionAborted, "Unexpected EOF"))
            };

            if byte == b'\n' && prev_byte_was_cr {
                buf.pop();  // removing the '\r'
                return AsciiString::from_ascii(buf)
                    .map_err(|_| IoError::new(ErrorKind::InvalidInput, "Header is not in ASCII"))
            }

            prev_byte_was_cr = byte == b'\r';

            buf.push(byte);
        }
    }

    /// Reads a request from the stream.
    /// Blocks until the header has been read.
    fn read(&mut self) -> Result<Request, ReadError> {
        let (method, path, version, headers) = {
            // reading the request line
            let (method, path, version) = {
                let line = try!(self.read_next_line().map_err(|e| ReadError::ReadIoError(e)));

                try!(parse_request_line(
                    line.as_str().trim()    // TODO: remove this conversion
                ))
            };

            // getting all headers
            let headers = {
                let mut headers = Vec::new();
                loop {
                    let line = try!(self.read_next_line().map_err(|e| ReadError::ReadIoError(e)));

                    if line.len() == 0 { break };
                    headers.push(
                        match FromStr::from_str(line.as_str().trim()) {    // TODO: remove this conversion
                            Ok(h) => h,
                            _ => return Err(ReadError::WrongHeader(version))
                        }
                    );
                }

                headers
            };

            (method, path, version, headers)
        };

        // building the writer for the request
        let writer = self.sink.next().unwrap();

        // follow-up for next potential request
        let mut data_source = self.source.next().unwrap();
        ::std::mem::swap(&mut self.next_header_source, &mut data_source);

        // building the next reader
        let request = try!(::request::new_request(self.secure, method, path, version.clone(),
                headers, self.remote_addr.as_ref().unwrap().clone(), data_source, writer)
            .map_err(|e| {
                use request;
                match e {
                    request::RequestCreationError::CreationIoError(e) => ReadError::ReadIoError(e),
                    request::RequestCreationError::ExpectationFailed => ReadError::ExpectationFailed(version)
                }
            }));

        // return the request
        Ok(request)
    }
}

impl Iterator for ClientConnection {
    type Item = Request;
    /// Blocks until the next Request is available.
    /// Returns None when no new Requests will come from the client.
    fn next(&mut self) -> Option<Request> {
        use {Response, StatusCode};

        // the client sent a "connection: close" header in this previous request
        //  or is using HTTP 1.0, meaning that no new request will come
        if self.no_more_requests {
            return None
        }

        loop {
            let rq = match self.read() {
                Err(ReadError::WrongRequestLine) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(400));
                    response.raw_print(writer, HTTPVersion(1, 1), &[], false, None).ok();
                    return None;    // we don't know where the next request would start,
                                    // se we have to close
                },

                Err(ReadError::WrongHeader(ver)) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(400));
                    response.raw_print(writer, ver, &[], false, None).ok();
                    return None;    // we don't know where the next request would start,
                                    // se we have to close
                },

                Err(ReadError::ReadIoError(ref err)) if err.kind() == ErrorKind::TimedOut => {
                    // request timeout
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(408));
                    response.raw_print(writer, HTTPVersion(1, 1), &[], false, None).ok();
                    return None;    // closing the connection
                },

                Err(ReadError::ExpectationFailed(ver)) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(417));
                    response.raw_print(writer, ver, &[], true, None).ok();
                    return None;    // TODO: should be recoverable, but needs handling in case of body
                },

                Err(ReadError::ReadIoError(_)) =>
                    return None,

                Ok(rq) => rq
            };

            // checking HTTP version
            if *rq.http_version() > (1, 1) {
                let writer = self.sink.next().unwrap();
                let response =
                    Response::from_string("This server only supports HTTP versions 1.0 and 1.1"
                        .to_owned()).with_status_code(StatusCode(505));
                response.raw_print(writer, HTTPVersion(1, 1), &[], false, None).ok();
                continue
            }

            // updating the status of the connection
            {
                let connection_header = rq.headers().iter()
                    .find(|h| h.field.equiv(&"Connection"))
                    .map(|h| AsRef::<str>::as_ref(h.value.as_ref()));

                let lowercase = connection_header.map(|h| h.to_ascii_lowercase());

                match lowercase {
                    Some(ref val) if val.contains("close") =>
                        self.no_more_requests = true,

                    Some(ref val) if val.contains("upgrade") =>
                        self.no_more_requests = true,

                    Some(ref val) if !val.contains("keep-alive") &&
                                    *rq.http_version() == HTTPVersion(1, 0) =>
                        self.no_more_requests = true,

                    None if *rq.http_version() == HTTPVersion(1, 0) =>
                        self.no_more_requests = true,

                    _ => ()
                };
            }

            // returning the request
            return Some(rq);
        }
    }
}

/// Parses a "HTTP/1.1" string.
fn parse_http_version(version: &str) -> Result<HTTPVersion, ReadError> {
    let elems = version.splitn(2, '/').map(|e| e.to_owned()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(ReadError::WrongRequestLine)
    }

    let elems = elems[1].splitn(2, '.')
        .map(|e| e.to_owned()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(ReadError::WrongRequestLine)
    }

    match (FromStr::from_str(&elems[0]), FromStr::from_str(&elems[1])) {
        (Ok(major), Ok(minor)) =>
            Ok(HTTPVersion(major, minor)),
        _ => Err(ReadError::WrongRequestLine)
    }
}

/// Parses the request line of the request.
/// eg. GET / HTTP/1.1
fn parse_request_line(line: &str) -> Result<(Method, String, HTTPVersion), ReadError> {
    let mut words = line.split(' ');

    let method = words.next();
    let path = words.next();
    let version = words.next();

    let (method, path, version) = match (method, path, version) {
        (Some(m), Some(p), Some(v)) => (m, p, v),
        _ => return Err(ReadError::WrongRequestLine)
    };

    let method = match FromStr::from_str(method) {
        Ok(method) => method,
        Err(()) => return Err(ReadError::WrongRequestLine)
    };

    let version = try!(parse_http_version(version));

    Ok((method, path.to_owned(), version))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_request_line() {
        let (method, path, ver) =
            match super::parse_request_line("GET /hello HTTP/1.1") {
                Err(_) => panic!(),
                Ok(v) => v
            };

        assert!(method == ::Method::Get);
        assert!(path == "/hello");
        assert!(ver == ::common::HTTPVersion(1, 1));

        assert!(super::parse_request_line("GET /hello").is_err());
        assert!(super::parse_request_line("qsd qsd qsd").is_err());
    }
}
