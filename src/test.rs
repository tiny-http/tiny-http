use crate::{request::new_request, Request};
use http::{header, HeaderMap, HeaderName, HeaderValue, Method, Uri, Version};
use std::net::SocketAddr;

/// A simpler version of [`Request`] that is useful for testing. No data actually goes anywhere.
///
/// By default, `TestRequest` pretends to be an insecure GET request for the server root (`/`)
/// with no headers. To create a `TestRequest` with different parameters, use the builder pattern:
///
/// ```
/// # use http::{Method, Uri};
/// # use tiny_http::TestRequest;
/// let request = TestRequest::new()
///     .with_method(Method::POST)
///     .with_path(Uri::from_static("/api/widgets"))
///     .with_body("42");
/// ```
///
/// Then, convert the `TestRequest` into a real `Request` and pass it to the server under test:
///
/// ```
/// # use http::{Method, Uri};
/// # use tiny_http::{Request, Response, Server, TestRequest};
/// # use std::io::Cursor;
/// # let request = TestRequest::new()
/// #     .with_method(Method::POST)
/// #     .with_path(Uri::from_static("/api/widgets"))
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
/// assert_eq!(response.status_code(), http::StatusCode::OK);
/// ```
pub struct TestRequest {
    body: &'static str,
    remote_addr: SocketAddr,
    // true if HTTPS, false if HTTP
    secure: bool,
    method: Method,
    path: Uri,
    http_version: Version,
    headers: HeaderMap,
}

impl From<TestRequest> for Request {
    fn from(mut mock: TestRequest) -> Request {
        // if the user didn't set the Content-Length header, then set it for them
        // otherwise, leave it alone (it may be under test)
        if let header::Entry::Vacant(vacant) = mock.headers.entry(header::CONTENT_TYPE) {
            vacant.insert(HeaderValue::from_str(&mock.body.len().to_string()).unwrap());
        }
        new_request(
            mock.secure,
            mock.method,
            mock.path,
            mock.http_version,
            mock.headers,
            Some(mock.remote_addr),
            mock.body.as_bytes(),
            std::io::sink(),
        )
        .unwrap()
    }
}

impl Default for TestRequest {
    fn default() -> Self {
        TestRequest {
            body: "",
            remote_addr: "127.0.0.1:23456".parse().unwrap(),
            secure: false,
            method: Method::GET,
            path: Uri::default(),
            http_version: Version::HTTP_11,
            headers: HeaderMap::new(),
        }
    }
}

impl TestRequest {
    pub fn new() -> Self {
        TestRequest::default()
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
    pub fn with_path(mut self, path: Uri) -> Self {
        self.path = path;
        self
    }
    pub fn with_http_version(mut self, version: Version) -> Self {
        self.http_version = version;
        self
    }
    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }
}
