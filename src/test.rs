use ascii::AsciiString;
use crate::{HeaderField, Method, HTTPVersion, Header, Request, request::new_request};
use std::net::SocketAddr;
use std::str::FromStr;

/// A simpler version of a `Request` that is useful for testing. No data actually goes anywhere.
///
/// By default, `MockRequest` pretends to be an unsecure GET request for the server root (`/`)
/// with no headers. To create a `MockRequest` with different parameters, use the builder pattern:
///
/// ```
/// # use tiny_http::{Method, MockRequest};
/// let request = MockRequest::new()
///     .with_method(Method::Post)
///     .with_path("/api/widgets")
///     .with_body("42");
/// ```
///
/// Then, convert the `MockRequest` into a real `Request` and pass it to the server under test:
///
/// ```
/// # use tiny_http::{Method, Request, Response, Server, StatusCode, MockRequest};
/// # use std::io::Cursor;
/// # let request = MockRequest::new()
/// #     .with_method(Method::Post)
/// #     .with_path("/api/widgets")
/// #     .with_body("42");
/// # struct TestServer {
/// #     listener: Server,
/// # }
/// # let server = TestServer {
/// #     listener: Server::http("0.0.0.0:0").unwrap(),
/// # };
/// # impl TestServer {
/// #     fn handle_request(&self, request: Request) -> Response<Cursor<Vec<u8>>> {
/// #         Response::from_string("test")
/// #     }
/// # }
/// let response = server.handle_request(request.into());
/// assert_eq!(response.status_code(), StatusCode(200));
/// ```
pub struct MockRequest {
    body: &'static str,
    remote_addr: SocketAddr,
    // true if HTTPS, false if HTTP
    secure: bool,
    method: Method,
    path: &'static str,
    http_version: HTTPVersion,
    headers: Vec<Header>,
}

impl From<MockRequest> for Request {
    fn from(mut mock: MockRequest) -> Request {
        // if the user didn't set the Content-Length header, then set it for them
        // otherwise, leave it alone (it may be under test)
        if mock
            .headers
            .iter_mut()
            .find(|h| h.field.equiv("Content-Length"))
            .is_none()
        {
            mock.headers.push(Header {
                field: HeaderField::from_str("Content-Length").unwrap(),
                value: AsciiString::from_ascii(mock.body.len().to_string()).unwrap(),
            });
        }
        new_request(
            mock.secure,
            mock.method,
            mock.path.to_string(),
            mock.http_version,
            mock.headers,
            mock.remote_addr,
            mock.body.as_bytes(),
            std::io::sink(),
        )
        .unwrap()
    }
}

impl Default for MockRequest {
    fn default() -> Self {
        MockRequest {
            body: "",
            remote_addr: "0.0.0.0:0".parse().unwrap(),
            secure: false,
            method: Method::Get,
            path: "/",
            http_version: HTTPVersion::from((1, 1)),
            headers: Vec::new(),
        }
    }
}

impl MockRequest {
    pub fn new() -> Self {
        MockRequest::default()
    }
    pub fn with_body(mut self, body: &'static str) -> Self {
        self.body = body;
        self
    }
    pub fn with_remote_addr(mut self, remote_addr: SocketAddr) -> Self {
        self.remote_addr = remote_addr;
        self
    }
    pub fn with_https(mut self) -> Self {
        self.secure = true;
        self
    }
    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }
    pub fn with_path(mut self, path: &'static str) -> Self {
        self.path = path;
        self
    }
    pub fn with_http_version(mut self, version: HTTPVersion) -> Self {
        self.http_version = version;
        self
    }
    pub fn with_header(mut self, header: Header) -> Self {
        self.headers.push(header);
        self
    }
}
