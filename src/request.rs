use std::io::IoError;
use std::io::net::ip;
use {Header, HTTPVersion, Method, Response, StatusCode};

/// Represents an HTTP request made by a client.
///
/// A `Request` object is what is produced by the server, and is your what
///  your code must analyse and answer.
///
/// This object implements the `Send` trait, therefore you can dispatch your requests to
///  worker threads.
///
/// # Pipelining
/// 
/// If a client sends multiple requests in a row (without waiting for the response), then you will
///  get multiple `Request` objects simultaneously. This is called *requests pipelining*.
/// Tiny-http automatically reorders the responses so that you don't need to worry about the order
///  in which you call `respond` or `into_writer`.
///
/// This mechanic is disabled if:
/// 
///  - The body of a request is large enough (handling requires pipelining requires storing the
///     body of the request in a buffer ; if the body is too big, tiny-http will avoid doing that)
///  - A request sends a `Expect: 100-continue` header (which means that the client waits to
///     know whether its body will be processed before sending it)
///
/// In these situations, you will only get one `Request` object per client at a time.
///
/// # Automatic cleanup
/// 
/// If a `Request` object is destroyed without `into_writer` or `respond` being called,
///  an empty response with a 500 status code (internal server error) will automatically be
///  sent back to the client.
/// This means that if your code fails during the handling of a request, this "internal server
///  error" response will automatically be sent during the stack unwinding.
#[unstable]
pub struct Request {
    // where to read the body from
    data_reader: Box<Reader + Send>,

    // if this writer is empty, then the request has been answered
    response_writer: Option<Box<Writer + Send>>,

    remote_addr: ip::SocketAddr,

    method: Method,

    path: ::url::Path,

    http_version: HTTPVersion,

    headers: Vec<Header>,

    body_length: Option<uint>,

    // true if a `100 Continue` response must be sent when `as_reader()` is called
    must_send_continue: bool,
}

/// Error that can happen when building a `Request` object.
pub enum RequestCreationError {
    ExpectationFailed,
    CreationIoError(IoError),
}

// this trait is to make sure that Request implements Send
trait MustBeSendDummy : Send {}
impl MustBeSendDummy for Request {}

/// Builds a new request
pub fn new_request<R: Reader + Send, W: Writer + Send>(method: Method, path: ::url::Path,
                             version: HTTPVersion, headers: Vec<Header>,
                             remote_addr: ip::SocketAddr, mut source_data: R, writer: W)
    -> Result<Request, RequestCreationError>
{
    // finding the transfer-encoding header
    let transfer_encoding = headers.iter()
        .find(|h: &&Header| h.field.equiv(&"Transfer-Encoding"))
        .map(|h| h.value.clone());

    // finding the content-length header
    let content_length = if transfer_encoding.is_some() {
        // if transfer-encoding is specified, the Content-Length
        //  header must be ignored (RFC2616 #4.4)
        None

    } else {
        headers.iter()
            .find(|h: &&Header| h.field.equiv(&"Content-Length"))
            .and_then(|h| from_str::<uint>(h.value.as_slice()))
    };

    // true if the client sent a `Expect: 100-continue` header
    let expects_continue = {
        use std::ascii::StrAsciiExt;

        match headers.iter().find(|h: &&Header| h.field.equiv(&"Expect")) {
            None => false,
            Some(h) if h.value.as_slice().eq_ignore_ascii_case("100-continue")
                => true,
            _ => return Err(ExpectationFailed)
        }
    };

    // building the reader depending on
    //  transfer-encoding and content-length
    let reader =
        if content_length.is_some() {
            let content_length = content_length.as_ref().unwrap().clone();

            if content_length == 0 {
                use std::io::util::NullReader;
                box NullReader as Box<Reader + Send>

            } else if content_length <= 1024 && !expects_continue {
                use std::io::MemReader;
                let data = try!(source_data.read_exact(content_length)
                    .map_err(|e| CreationIoError(e)));
                box MemReader::new(data) as Box<Reader + Send>

            } else {
                use util::EqualReader;
                let (data_reader, _) = EqualReader::new(source_data, content_length);   // TODO:
                box data_reader as Box<Reader + Send>
            }

        } else if transfer_encoding.is_some() {
            // if a transfer-encoding was specified, then "chunked"
            //  is ALWAYS applied over the message (RFC2616 #3.6)
            use util::ChunksDecoder;
            box ChunksDecoder::new(source_data) as Box<Reader + Send>

        } else {
            // if we have neither a Content-Length nor a Transfer-Encoding,
            //  assuming that we have no data
            // TODO: could also be multipart/byteranges
            use std::io::util::NullReader;
            box NullReader as Box<Reader + Send>
        };

    Ok(Request {
        data_reader: reader,
        response_writer: Some(box writer as Box<Writer + Send>),
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
    #[stable]
    #[inline]
    pub fn get_method<'a>(&'a self) -> &'a Method {
        &self.method
    }

    /// Returns the resource requested by the client.
    #[unstable]
    #[inline]
    pub fn get_url<'a>(&'a self) -> &'a ::url::Path {
        &self.path
    }

    /// Returns a list of all headers sent by the client.
    #[stable]
    #[inline]
    pub fn get_headers<'a>(&'a self) -> &'a [Header] {
        self.headers.as_slice()
    }

    /// Returns the HTTP version of the request.
    #[unstable]
    #[inline]
    pub fn get_http_version<'a>(&'a self) -> &'a HTTPVersion {
        &self.http_version
    }

    /// Returns the length of the body in bytes.
    ///
    /// Returns `None` if the length is unknown.
    #[unstable]
    #[inline]
    pub fn get_body_length(&self) -> Option<uint> {
        self.body_length
    }

    /// Returns the length of the body in bytes.
    #[stable]
    #[inline]
    pub fn get_remote_addr<'a>(&'a self) -> &'a ip::SocketAddr {
        &self.remote_addr
    }

    /// Allows to read the body of the request.
    /// 
    /// # Example
    /// 
    /// ```
    /// let request = server.recv();
    /// 
    /// if get_content_type(&request) == "application/json" {
    ///     let json: Json = from_str(request.as_reader().read_to_string()).unwrap();
    /// }
    /// ```
    ///
    /// If the client sent a `Expect: 100-continue` header with the request, calling this
    ///  function will send back a `100 Continue` response.
    #[unstable]
    #[inline]
    pub fn as_reader<'a>(&'a mut self) -> &'a mut Reader {
        if self.must_send_continue {
            fn pt<'a>(w: &'a mut Writer) -> &'a mut Writer { w }

            let msg = Response::new_empty(StatusCode(100));
            msg.raw_print(pt(*self.response_writer.as_mut().unwrap()),
                self.http_version, self.headers.as_slice(), true).ok();
            self.response_writer.as_mut().unwrap().flush().ok();
            self.must_send_continue = false;
        }

        fn passthrough<'a>(r: &'a mut Reader) -> &'a mut Reader { r }
        passthrough(self.data_reader)
    }

    /// Turns the `Request` into a writer.
    /// 
    /// The writer has a raw access to the stream to the user.
    /// This function is useful for things like CGI.
    ///
    /// Note that the destruction of the `Writer` object may trigger
    ///  some events. For exemple if a client has sent multiple requests and the requests
    ///  have been processed in parallel, the destruction of a writer will trigger
    ///  the writing of the next response.
    /// Therefore you should always destroy the `Writer` as soon as possible.
    #[stable]
    #[inline]
    pub fn into_writer(mut self) -> Box<Writer + Send> {
        self.into_writer_impl()
    }

    fn into_writer_impl(&mut self) -> Box<Writer + Send> {
        use std::mem;

        assert!(self.response_writer.is_some());

        let mut writer = None;
        mem::swap(&mut self.response_writer, &mut writer);
        writer.unwrap()
    }

    /// Sends a response to this request.
    #[unstable]
    #[inline]
    pub fn respond<R: Reader>(mut self, response: Response<R>) {
        self.respond_impl(response)
    }

    fn respond_impl<R: Reader>(&mut self, response: Response<R>) {
        use std::io;

        fn passthrough<'a>(w: &'a mut Writer) -> &'a mut Writer { w }
        let mut writer = self.into_writer_impl();

        let do_not_send_body = self.method.equiv(&"HEAD");

        match response.raw_print(passthrough(writer),
                                self.http_version, self.headers.as_slice(),
                                do_not_send_body)
        {
            Ok(_) => (),
            Err(ref err) if err.kind == io::Closed => (),
            Err(ref err) if err.kind == io::BrokenPipe => (),
            Err(ref err) if err.kind == io::ConnectionAborted => (),
            Err(ref err) if err.kind == io::ConnectionRefused => (),
            Err(ref err) if err.kind == io::ConnectionReset => (),
            Err(ref err) =>
                println!("error while sending answer: {}", err)     // TODO: handle better?
        };

        writer.flush().ok();
    }
}

impl ::std::fmt::Show for Request {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter)
        -> Result<(), ::std::fmt::FormatError>
    {
        (format!("Request({} {} from {})",
            self.method, self.path, self.remote_addr.ip)).fmt(formatter)
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
