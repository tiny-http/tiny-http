use common::{Header, HTTPVersion, StatusCode};
use std::io::{IoResult, MemReader};
use std::io::fs::File;
use std::io::util;
use std::io::util::NullReader;

/// Object representing an HTTP response whose purpose is to be given to a `Request`.
/// 
/// Some headers cannot be changed. Trying to define the value
///  of one of these will have no effect:
/// 
///  - `Accept-Ranges`
///  - `Connection`
///  - `Content-Range`
///  - `Trailer`
///  - `Transfer-Encoding`
///  - `Upgrade`
///
/// Some headers have special behaviors:
/// 
///  - `Content-Encoding`: If you define this header, the library
///      will assume that the data from the `Reader` has the specified encoding
///      and will just pass-through.
///  - `Content-Length`: If you define this header to `N`, only the first `N` bytes
///      of the `Reader` will be read. If the `Reader` reaches `EOF` before `N` bytes have
///      been read, `0`s will be sent. Also, this header may not be passed to the final
///      output.
///
#[experimental]
pub struct Response<R> {
    reader: R,
    status_code: StatusCode,
    headers: Vec<Header>,
    data_length: Option<uint>,
}

/// Transfer encoding to use when sending the message.
/// Note that only *supported* encoding are listed here.
enum TransferEncoding {
    Identity,
    Chunked,
}

/// Builds a Date: header with the current date.
// TODO: this is optimisable
fn build_date_header() -> Header {
    use time;
    let date = time::now_utc().strftime("%a, %d %b %Y %H:%M:%S GMT");
    from_str(format!("Date: {}", date).as_slice()).unwrap()
}

fn write_message_header<W: Writer>(mut writer: W, http_version: &HTTPVersion,
                                   status_code: &StatusCode, headers: &[Header])
    -> IoResult<()>
{
    // writing status line
    try!(write!(writer, "HTTP/{} {} {}\r\n",
        http_version,
        status_code.as_uint(),
        status_code.get_default_reason_phrase()
    ));

    // writing headers
    for header in headers.iter() {
        try!(write!(writer, "{}: {}\r\n", header.field, header.value));
    }

    // separator between header and data
    try!(write!(writer, "\r\n"));

    Ok(())
}

fn choose_transfer_encoding(request_headers: &[Header], http_version: &HTTPVersion,
                            entity_length: &Option<uint>)
    -> TransferEncoding
{
    // HTTP 1.0 doesn't support other encodings
    if *http_version <= HTTPVersion(1, 0) {
        return Identity;
    }

    // TODO: parse the request's TE header

    // if we don't have a Content-Length, or if the Content-Length is too big, using chunks writer
    let chunks_threshold = 32768;
    if entity_length.as_ref().filtered(|val| **val < chunks_threshold).is_none() {
        return Chunked;
    }

    // Identity by default
    Identity
}

impl<R: Reader> Response<R> {
    #[experimental]
    pub fn new(status_code: StatusCode, headers: Vec<Header>,
               data: R, data_length: Option<uint>) -> Response<R>
    {
        let mut response = Response {
            reader: data,
            status_code: status_code,
            headers: Vec::new(),
            data_length: data_length,
        };

        for h in headers.move_iter() {
            response.add_header(h)
        }

        response
    }

    /// Adds a header to the list.
    /// Does all the checks.
    #[experimental]
    pub fn add_header(&mut self, header: Header) {
        // ignoring forbidden headers
        if header.field.equiv(&"Accept-Ranges") ||
           header.field.equiv(&"Connection") ||
           header.field.equiv(&"Content-Range") ||
           header.field.equiv(&"Trailer") ||
           header.field.equiv(&"Transfer-Encoding") ||
           header.field.equiv(&"Upgrade")
        {
            return;
        }

        // if the header is Content-Length, setting the data length
        if header.field.equiv(&"Content-Length") {
            match from_str::<uint>(header.value.as_slice()) {
                Some(val) => self.data_length = Some(val),
                None => ()      // wrong value for content-length
            };

            return;
        }

        self.headers.push(header)
    }

    /// Returns the same request, but with an additional header.
    ///
    /// Some headers cannot be modified and some other have a
    ///  special behavior. See the documentation above.
    #[unstable]
    pub fn with_header(mut self, header: Header) -> Response<R> {
        self.add_header(header);
        self
    }

    /// Returns the same request, but with a different status code.
    #[unstable]
    pub fn with_status_code(mut self, code: StatusCode) -> Response<R> {
        self.status_code = code;
        self
    }

    /// Prints the HTTP response to a writer.
    ///
    /// This function is the one used to send the response to the client's socket.
    /// Therefore you shouldn't expect anything pretty-printed or even readable.
    ///
    /// The HTTP version and headers passed as arguments are used to
    ///  decide which features (most notably, encoding) to use.
    // TODO: status codes 1xx, 204 and 304 must not include a response body
    #[unstable]
    pub fn raw_print<W: Writer>(mut self, mut writer: W, http_version: HTTPVersion,
                                request_headers: &[Header]) -> IoResult<()>
    {
        let transfer_encoding = choose_transfer_encoding(request_headers,
                                    &http_version, &self.data_length);

        // add `Date` if not in the headers
        if self.headers.iter().find(|h| h.field.equiv(&"Date")).is_none() {
            self.headers.unshift(build_date_header());
        }

        // add `Server` if not in the headers
        if self.headers.iter().find(|h| h.field.equiv(&"Server")).is_none() {
            self.headers.unshift(
                from_str("Server: tiny-http (Rust)").unwrap()
            );
        }

        // writing data
        match transfer_encoding {
            Chunked => {
                use util::ChunksEncoder;

                self.headers.push(
                    from_str("Transfer-Encoding: chunked").unwrap()
                );

                try!(write_message_header(writer.by_ref(), &http_version,
                    &self.status_code, self.headers.as_slice()));

                let mut writer = ChunksEncoder::new(writer);
                try!(util::copy(&mut self.reader, &mut writer));
                
                // flushing!
                try!(writer.flush());
            },

            Identity => {
                use util::EqualReader;

                assert!(self.data_length.is_some());
                let data_length = self.data_length.unwrap();

                self.headers.push(
                    from_str(format!("Content-Length: {}", data_length).as_slice()).unwrap()
                );

                try!(write_message_header(writer.by_ref(), &http_version,
                    &self.status_code, self.headers.as_slice()));

                if data_length >= 1 {
                    let (mut equ_reader, _) =
                        EqualReader::new(self.reader.by_ref(), data_length);
                    try!(util::copy(&mut equ_reader, &mut writer));
                }

                // flushing!
                try!(writer.flush());
            }
        };

        Ok(())
    }
}

impl Response<File> {
    /// Builds a new `Response` from a `File`.
    ///
    /// The `Content-Type` will **not** be automatically detected,
    ///  you must set it yourself.
    #[experimental]
    pub fn from_file(mut file: File) -> Response<File> {
        let file_size = file.stat().ok().map(|v| v.size as uint);

        Response::new(
            StatusCode(200),
            Vec::new(),
            file,
            file_size
        )
    }
}

impl Response<MemReader> {
    #[experimental]
    pub fn from_data(data: Vec<u8>) -> Response<MemReader> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            Vec::new(),
            MemReader::new(data),
            Some(data_len)
        )
    }

    #[experimental]
    pub fn from_string(data: String) -> Response<MemReader> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            vec!(
                from_str("Content-Type: text/plain; charset=UTF-8").unwrap()
            ),
            MemReader::new(data.into_bytes()),
            Some(data_len)
        )        
    }
}

impl Response<NullReader> {
    /// Builds an empty `Response` with the given status code.
    #[experimental]
    pub fn new_empty(status_code: StatusCode) -> Response<NullReader> {
        Response::new(
            status_code,
            Vec::new(),
            NullReader,
            Some(0)
        )
    }
}
