use std::io;
use std::io::IoResult;
use std::io::net::tcp;
use std::io::net::ip::SocketAddr;
use std::io::BufferedReader;
use common::{Header, HTTPVersion, Method};
use Request;
use url::Path;
use sequential::{SequentialReader, SequentialReaderBuilder, SequentialWriterBuilder};

/// A ClientConnection is an object that will store a socket to a client
/// and return Request objects.
pub struct ClientConnection {
    // address of the client
    remote_addr: io::IoResult<SocketAddr>,

    // sequence of Readers to the stream, so that the data is not read in
    //  the wrong order
    source: SequentialReaderBuilder<tcp::TcpStream>,

    // sequence of Writers to the stream, to avoid writing response #2 before
    //  response #1
    sink: SequentialWriterBuilder<tcp::TcpStream>,

    // Reader to read the next header from
	next_header_source: SequentialReader<tcp::TcpStream>,

    // set to true if the client sent a "Connection: close" in the previous request
    connection_must_close: bool,
}

impl ClientConnection {
    /// Creates a new ClientConnection that takes ownership of the TcpStream.
    pub fn new(mut socket: tcp::TcpStream) -> ClientConnection {
        socket.set_timeout(Some(10000));

        let remote_addr = socket.peer_name();

        let mut source = SequentialReaderBuilder::new(socket.clone());
        let first_header = source.next().unwrap();

        ClientConnection {
            source: source,
            sink: SequentialWriterBuilder::new(socket),
            remote_addr: remote_addr,
            next_header_source: first_header,
            connection_must_close: false,
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
    fn read(&mut self) -> io::IoResult<Request> {
        use util::EqualReader;

        let (method, path, version, headers) = {
            // reading the request line
            let (method, path, version) = {
                let line = try!(self.read_next_line());

                try!(parse_request_line(
                    line.as_slice().trim()
                ))
            };

            // getting all headers
            let headers = {
                let mut headers = Vec::new();
                loop {
                    let line = try!(self.read_next_line());

                    if line.as_slice().trim().len() == 0 { break };
                    headers.push(
                        match from_str(line.as_slice().trim()) {
                            Some(h) => h,
                            None => return Err(gen_invalid_input(
                                "Could not parse header"))
                        }
                    );
                }
                headers
            };

            (method, path, version, headers)
        };

        // finding length of body
        let body_length = headers.iter()
            .find(|h: &&Header| h.field.equiv(&"Content-Length"))
            .and_then(|h| from_str::<uint>(h.value.as_slice()))
            .unwrap_or(0u);

        // building the next reader
        let data_reader = self.source.next().unwrap();
        let (data_reader, _) = EqualReader::new(data_reader, body_length);   // TODO:
        self.next_header_source = self.source.next().unwrap();

        // building the writer
        let writer = self.sink.next().unwrap();

        // building the request
        Ok(Request {
            data_reader: box data_reader,
            response_writer: box writer,
            remote_addr: self.remote_addr.clone().unwrap(),     // TODO: could fail
            method: method,
            path: path,
            http_version: version,
            headers: headers,
            body_length: body_length,
        })
    }
}

impl Iterator<Request> for ClientConnection {
    /// Blocks until the next Request is available.
    /// Returns None when no new Requests will come from the client.
    fn next(&mut self) -> Option<Request> {
        // the client sent a "connection: close" header in this previous request
        //  or is using HTTP 1.0, meaning that no new request will come
        if self.connection_must_close {
            return None
        }

        // TODO: send back message to client in case of parsing error
        loop {
            let rq = match self.read() {
                Err(_) => return None,
                Ok(rq) => rq
            };

            // updating the status of the connection
            {
                let connection_header = rq.headers.iter()
                    .find(|h| h.field.equiv(&"Connection")).map(|h| h.value.as_slice());

                if connection_header == Some("close") {
                    self.connection_must_close = true;
                } else if rq.http_version == HTTPVersion(1, 0) &&
                        connection_header != Some("keep-alive")
                {
                    self.connection_must_close = true;
                }
            }

            // returning the request
            return Some(rq);
        }
    }
}

/// Generates an IoError with invalid input.
/// This function is here because it is incredibly annoying to create this error.
fn gen_invalid_input(desc: &'static str) -> io::IoError {
    io::IoError {
        kind: io::InvalidInput,
        desc: desc,
        detail: None
    }
}

/// Parses a "HTTP/1.1" string.
fn parse_http_version(version: &str) -> io::IoResult<HTTPVersion> {
    let elems = version.splitn('/', 2).map(|e| e.to_string()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(gen_invalid_input("Wrong HTTP version format"))
    }

    let elems = elems.get(1).as_slice().splitn('.', 2)
        .map(|e| e.to_string()).collect::<Vec<String>>();
    if elems.len() != 2 {
        return Err(gen_invalid_input("Wrong HTTP version format"))
    }

    match (from_str(elems.get(0).as_slice()), from_str(elems.get(1).as_slice())) {
        (Some(major), Some(minor)) =>
            Ok(HTTPVersion(major, minor)),
        _ => Err(gen_invalid_input("Wrong HTTP version format"))
    }
}

/// Parses the request line of the request.
/// eg. GET / HTTP/1.1
fn parse_request_line(line: &str) -> io::IoResult<(Method, Path, HTTPVersion)> {
    let mut words = line.words();

    let method = words.next();
    let path = words.next();
    let version = words.next();

    let (method, path, version) = match (method, path, version) {
        (Some(m), Some(p), Some(v)) => (m, p, v),
        _ => return Err(gen_invalid_input("Missing element in request line"))
    };

    let method = match from_str(method) {
        Some(method) => method,
        None => return Err(gen_invalid_input("Could not parse method"))
    };

    let path = match Path::parse(path) {
        Ok(p) => p,
        Err(_) => return Err(gen_invalid_input("Wrong requested URL"))
    };

    let version = try!(parse_http_version(version));

    Ok((method, path, version))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_request_line() {
        let (method, path, ver) =
            super::parse_request_line("GET /hello HTTP/1.1").unwrap();

        assert!(method.equiv(&"get"));
        assert!(path == from_str("/hello").unwrap());
        assert!(ver == ::common::HTTPVersion(1, 1));

        assert!(super::parse_request_line("GET /hello").is_err());
        assert!(super::parse_request_line("qsd qsd qsd").is_err());
    }
}
