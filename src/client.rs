use std::io;
use std::io::net::tcp;
use std::io::BufferedReader;
use std::io::util::LimitReader;
use common::{Header, Method};
use Request;
use url::Path;
use semver::Version;

pub struct ClientConnection {
    initial_socket: tcp::TcpStream,         // copy of the socket to be passed to request objects
	socket: BufferedReader<tcp::TcpStream>,
}

impl ClientConnection {
    pub fn new(socket: tcp::TcpStream) -> ClientConnection {
        ClientConnection {
            initial_socket: socket.clone(),
            socket: BufferedReader::new(socket),
        }
    }

    fn gen_invalid_input(desc: &'static str) -> io::IoError {
        io::IoError {
            kind: io::InvalidInput,
            desc: desc,
            detail: None
        }
    }

    /// Parses a "HTTP/1.1" string.
    fn parse_http_version(version: &str) -> io::IoResult<Version> {
        let elems = version.splitn('/', 2).map(|e| e.to_string()).collect::<Vec<String>>();
        if elems.len() != 2 {
            return Err(ClientConnection::gen_invalid_input("Wrong HTTP version format"))
        }

        let elems = elems.get(1).as_slice().splitn('.', 2)
            .map(|e| e.to_string()).collect::<Vec<String>>();
        if elems.len() != 2 {
            return Err(ClientConnection::gen_invalid_input("Wrong HTTP version format"))
        }

        match (from_str(elems.get(0).as_slice()), from_str(elems.get(1).as_slice())) {
            (Some(major), Some(minor)) =>
                Ok(Version {
                    major: major,
                    minor: minor,
                    patch: 0,
                    pre: Vec::new(),
                    build: Vec::new()
                }),
            _ => Err(ClientConnection::gen_invalid_input("Wrong HTTP version format"))
        }
    }

    /// Parses the first line of the request.
    /// eg. GET / HTTP/1.1
    fn parse_first_line(line: &str) -> io::IoResult<(Method, Path, Version)> {
        let mut words = line.words();

        let method = words.next();
        let path = words.next();
        let version = words.next();

        let (method, path, version) = match (method, path, version) {
            (Some(m), Some(p), Some(v)) => (m, p, v),
            _ => return Err(ClientConnection::gen_invalid_input("Missing element in first line"))
        };

        let method = match from_str(method) {
            Some(method) => method,
            None => return Err(ClientConnection::gen_invalid_input("Could not parse method"))
        };

        let path = match Path::parse(path) {
            Ok(p) => p,
            Err(_) => return Err(ClientConnection::gen_invalid_input("Wrong requested URL"))
        };

        let version = try!(ClientConnection::parse_http_version(version));

        Ok((method, path, version))
    }

    /// Parses a header line.
    /// eg. Host: example.com
    fn parse_header(line: &str) -> io::IoResult<Header> {
        let elems = line.splitn(':', 2).map(|e| e.to_string()).collect::<Vec<String>>();

        if elems.len() <= 1 {
            return Err(ClientConnection::gen_invalid_input(
                "Wrong header format (no ':')"))
        }
        if elems.get(1).as_slice().chars().next() != Some(' ') {
            return Err(ClientConnection::gen_invalid_input(
                "Wrong header format (missing space after ':')"))
        }

        let field = match from_str(elems.get(0).as_slice()) {
            None => return Err(ClientConnection::gen_invalid_input("Could not parse header")),
            Some(f) => f
        };

        Ok(Header {
            field: field,
            value: elems.get(1).as_slice().slice_from(1).to_string()
        })
    }

    /// Reads a request from the stream.
    /// Blocks until the header has been read.
    fn read(&mut self) -> io::IoResult<Request> {
        let mut lines = self.socket.lines();

        let (method, path, version) =
            try!(ClientConnection::parse_first_line(
                match lines.next() {
                    Some(line) => try!(line),
                    None => return Err(ClientConnection::gen_invalid_input(
                                "Missing first line of request"))
                }.as_slice().trim()
            ));

        let headers = {
            let mut headers = Vec::new();
            loop {
                match lines.next() {
                    Some(line) => {
                        let line = try!(line);
                        if line.as_slice().trim().len() == 0 { break };
                        headers.push(
                            try!(ClientConnection::parse_header(line.as_slice().trim()))
                        )
                    },
                    None => break
                }
            }
            headers
        };

        Ok(Request {
            read_socket: LimitReader::new(
                        BufferedReader::new(self.initial_socket.clone()), 1024
                    ),
            write_socket: self.initial_socket.clone(),
            method: method,
            path: path,
            http_version: version,
            headers: headers,
        })
    }
}

impl Iterator<Request> for ClientConnection {
    fn next(&mut self) -> Option<Request> {
        // TODO: send back message to client
        loop {
            return match self.read() {
                Err(error) => None,
                Ok(rq) => Some(rq)
            }
        }
    }
}
