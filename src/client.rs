use std::io;
use std::io::{BufferedReader, BufferedWriter, IoError, IoResult};
use std::io::net::ip::SocketAddr;
use common::{HTTPVersion, Method};
use Request;
use url::Path;
use util::{SequentialReader, SequentialReaderBuilder, SequentialWriterBuilder};
use util::ClosableTcpStream;

/// A ClientConnection is an object that will store a socket to a client
/// and return Request objects.
pub struct ClientConnection {
    // address of the client
    remote_addr: io::IoResult<SocketAddr>,

    // sequence of Readers to the stream, so that the data is not read in
    //  the wrong order
    source: SequentialReaderBuilder<BufferedReader<ClosableTcpStream>>,

    // sequence of Writers to the stream, to avoid writing response #2 before
    //  response #1
    sink: SequentialWriterBuilder<BufferedWriter<ClosableTcpStream>>,

    // Reader to read the next header from
	next_header_source: SequentialReader<BufferedReader<ClosableTcpStream>>,

    // set to true if we know that the previous request is the last one
    no_more_requests: bool,
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
    pub fn new(write_socket: ClosableTcpStream, mut read_socket: ClosableTcpStream)
        -> ClientConnection
    {
        let remote_addr = read_socket.peer_name();

        let mut source = SequentialReaderBuilder::new(BufferedReader::new(read_socket));
        let first_header = source.next().unwrap();

        ClientConnection {
            source: source,
            sink: SequentialWriterBuilder::new(BufferedWriter::new(write_socket)),
            remote_addr: remote_addr,
            next_header_source: first_header,
            no_more_requests: false,
        }
    }

    /// Reads the next line from self.next_header_source.
    /// 
    /// Reads until `CRLF` is reached. The next read will start
    ///  at the first byte of the new line.
    fn read_next_line(&mut self) -> IoResult<String> {
        use std::io;
        use std::path::BytesContainer;

        let mut buf = Vec::new();
        let mut prev_byte_was_cr = false;

        loop {
            let byte = try!(self.next_header_source.read_byte());

            if byte == b'\n' && prev_byte_was_cr {
                return match buf.container_as_str() {
                    Some(s) => Ok(s.to_string()),
                    None => Err(io::standard_error(io::InvalidInput))
                }
            }

            if byte == b'\r' {
                prev_byte_was_cr = true;
            } else {
                prev_byte_was_cr = false;
            }

            buf.push(byte);
        }
    }

    /// Reads a request from the stream.
    /// Blocks until the header has been read.
    fn read(&mut self) -> Result<Request, ReadError> {
        let (method, path, version, headers) = {
            // reading the request line
            let (method, path, version) = {
                let line = try!(self.read_next_line().map_err(|e| ReadIoError(e)));

                try!(parse_request_line(
                    line.as_slice().trim()
                ))
            };

            // getting all headers
            let headers = {
                let mut headers = Vec::new();
                loop {
                    let line = try!(self.read_next_line().map_err(|e| ReadIoError(e)));

                    if line.as_slice().trim().len() == 0 { break };
                    headers.push(
                        match from_str(line.as_slice().trim()) {
                            Some(h) => h,
                            None => return Err(WrongHeader(version))
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
        let request = try!(::request::new_request(method, path, version,
                headers, self.remote_addr.clone().unwrap(), data_source, writer)
            .map_err(|e| {
                use request;
                match e {
                    request::CreationIoError(e) => ReadIoError(e),
                    request::ExpectationFailed => ExpectationFailed(version)
                }
            }));

        // return the request
        Ok(request)
    }
}

impl Iterator<Request> for ClientConnection {
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
            use std::io::TimedOut;

            let rq = match self.read() {
                Err(WrongRequestLine) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(400));
                    response.raw_print(writer, HTTPVersion(1, 1), &[], false).ok();
                    return None;    // we don't know where the next request would start,
                                    // se we have to close
                },

                Err(WrongHeader(ver)) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(400));
                    response.raw_print(writer, ver, &[], false).ok();
                    return None;    // we don't know where the next request would start,
                                    // se we have to close
                },

                Err(ReadIoError(ref err)) if err.kind == TimedOut => {
                    // request timeout
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(408));
                    response.raw_print(writer, HTTPVersion(1, 1), &[], false).ok();
                    return None;    // closing the connection
                },

                Err(ExpectationFailed(ver)) => {
                    let writer = self.sink.next().unwrap();
                    let response = Response::new_empty(StatusCode(417));
                    response.raw_print(writer, ver, &[], true).ok();
                    return None;    // TODO: should be recoverable, but needs handling in case of body
                },

                Err(ReadIoError(_)) =>
                    return None,

                Ok(rq) => rq
            };

            // checking HTTP version
            if *rq.get_http_version() > HTTPVersion(1, 1) {
                let writer = self.sink.next().unwrap();
                let response =
                    Response::from_string("This server only supports HTTP versions 1.0 and 1.1"
                        .to_string()).with_status_code(StatusCode(505));
                response.raw_print(writer, HTTPVersion(1, 1), &[], false).ok();
                continue
            }

            // updating the status of the connection
            {
                use std::ascii::StrAsciiExt;

                let connection_header = rq.get_headers().iter()
                    .find(|h| h.field.equiv(&"Connection")).map(|h| h.value.as_slice());

                match connection_header {
                    Some(ref val) if val.eq_ignore_ascii_case("close") => 
                        self.no_more_requests = true,

                    Some(ref val) if val.eq_ignore_ascii_case("upgrade") => 
                        self.no_more_requests = true,

                    Some(ref val) if !val.eq_ignore_ascii_case("keep-alive") &&
                                    *rq.get_http_version() == HTTPVersion(1, 0) =>
                        self.no_more_requests = true,

                    None if *rq.get_http_version() == HTTPVersion(1, 0) =>
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
    let elems = version.splitn('/', 1).map(|e| e.to_string()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(WrongRequestLine)
    }

    let elems = elems[1].as_slice().splitn('.', 1)
        .map(|e| e.to_string()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(WrongRequestLine)
    }

    match (from_str(elems[0].as_slice()), from_str(elems[1].as_slice())) {
        (Some(major), Some(minor)) =>
            Ok(HTTPVersion(major, minor)),
        _ => Err(WrongRequestLine)
    }
}

/// Parses the request line of the request.
/// eg. GET / HTTP/1.1
fn parse_request_line(line: &str) -> Result<(Method, Path, HTTPVersion), ReadError> {
    let mut words = line.words();

    let method = words.next();
    let path = words.next();
    let version = words.next();

    let (method, path, version) = match (method, path, version) {
        (Some(m), Some(p), Some(v)) => (m, p, v),
        _ => return Err(WrongRequestLine)
    };

    let method = match from_str(method) {
        Some(method) => method,
        None => return Err(WrongRequestLine)
    };

    let path = match Path::parse(path) {
        Ok(p) => p,
        Err(_) => return Err(WrongRequestLine)
    };

    let version = try!(parse_http_version(version));

    Ok((method, path, version))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_request_line() {
        let (method, path, ver) =
            match super::parse_request_line("GET /hello HTTP/1.1") {
                Err(_) => fail!(),
                Ok(v) => v
            };

        assert!(method.equiv(&"get"));
        assert!(path == from_str("/hello").unwrap());
        assert!(ver == ::common::HTTPVersion(1, 1));

        assert!(super::parse_request_line("GET /hello").is_err());
        assert!(super::parse_request_line("qsd qsd qsd").is_err());
    }
}
