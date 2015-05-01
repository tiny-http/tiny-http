use common::{Header, HTTPVersion, StatusCode};

use std::ascii::AsciiExt;
use std::cmp::Ordering;
use std::sync::mpsc::Receiver;

use std::io::{self, Read, Write, Cursor};
use std::io::Result as IoResult;

use std::fs::File;

use std::str::FromStr;

/// Object representing an HTTP response whose purpose is to be given to a `Request`.
///
/// Some headers cannot be changed. Trying to define the value
/// of one of these will have no effect:
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
///     will assume that the data from the `Read` object has the specified encoding
///     and will just pass-through.
///
///  - `Content-Length`: The length of the data should be set manually
///     using the `Reponse` object's API. Attempting to set the value of this
///     header will be equivalent to modifying the size of the data but the header
///     itself may not be present in the final result.
///
pub struct Response<R> where R: Read {
    reader: R,
    status_code: StatusCode,
    headers: Vec<Header>,
    data_length: Option<usize>,
}

/// Transfer encoding to use when sending the message.
/// Note that only *supported* encoding are listed here.
enum TransferEncoding {
    Identity,
    Chunked,
}

impl FromStr for TransferEncoding {
    type Err = ();

    fn from_str(input: &str) -> Result<TransferEncoding, ()> {
        if input.eq_ignore_ascii_case("identity") {
            Ok(TransferEncoding::Identity)
        } else if input.eq_ignore_ascii_case("chunked") {
            Ok(TransferEncoding::Chunked)
        } else {
            Err(())
        }
    }
}

/// Builds a Date: header with the current date.
// TODO: this is optimisable
fn build_date_header() -> Header {
    // FIXME: right date
    FromStr::from_str("Date: Wed, 15 Nov 1995 06:25:24 GMT").unwrap()
}

fn write_message_header<W>(mut writer: W, http_version: &HTTPVersion,
                           status_code: &StatusCode, headers: &[Header])
                           -> IoResult<()> where W: Write
{
    // writing status line
    try!(write!(&mut writer, "HTTP/{} {} {}\r\n",
        http_version,
        status_code.as_u16(),
        status_code.get_default_reason_phrase()
    ));

    // writing headers
    for header in headers.iter() {
        try!(write!(&mut writer, "{}: {}\r\n", header.field.as_str().as_str(),
            header.value.as_str()));
    }

    // separator between header and data
    try!(write!(&mut writer, "\r\n"));

    Ok(())
}

fn choose_transfer_encoding(request_headers: &[Header], http_version: &HTTPVersion,
                            entity_length: &Option<usize>, has_additional_headers: bool)
    -> TransferEncoding
{
    use util;

    // HTTP 1.0 doesn't support other encoding
    if *http_version <= HTTPVersion(1, 0) {
        return TransferEncoding::Identity;
    }

    // parsing the request's TE header
    let user_request = request_headers.iter()
        // finding TE
        .find(|h| h.field.equiv(&"TE"))

        // getting its value
        .map(|h| h.value.clone())

        // getting the corresponding TransferEncoding
        .and_then(|value| {
            // getting list of requested elements
            let mut parse = util::parse_header_value(value.as_str());     // TODO: remove conversion

            // sorting elements by most priority
            parse.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

            // trying to parse each requested encoding
            for value in parse.iter() {
                // q=0 are ignored
                if value.1 <= 0.0 { continue }

                match <TransferEncoding as FromStr>::from_str(value.0) {
                    Ok(te) => return Some(te),
                    _ => ()     // unrecognized/unsupported encoding
                };
            }

            // encoding not found
            None
        });

    //
    if user_request.is_some() {
        return user_request.unwrap();
    }

    // if we have additional headers, using chunked
    if has_additional_headers {
        return TransferEncoding::Chunked;
    }

    // if we don't have a Content-Length, or if the Content-Length is too big, using chunks writer
    let chunks_threshold = 32768;
    if entity_length.as_ref().map_or(true, |val| *val >= chunks_threshold) {
        return TransferEncoding::Chunked;
    }

    // Identity by default
    TransferEncoding::Identity
}

impl<R> Response<R> where R: Read {
    /// Creates a new Response object.
    ///
    /// The `additional_headers` argument is a receiver that
    ///  may provide headers even after the response has been sent.
    ///
    /// All the other arguments are straight-forward.
    pub fn new(status_code: StatusCode, headers: Vec<Header>,
               data: R, data_length: Option<usize>,
               additional_headers: Option<Receiver<Header>>)
                -> Response<R>
    {
        let mut response = Response {
            reader: data,
            status_code: status_code,
            headers: Vec::new(),
            data_length: data_length,
        };

        for h in headers.into_iter() {
            response.add_header(h)
        }

        // dummy implementation
        if additional_headers.is_some() {
            for h in additional_headers.unwrap().iter() {
                response.add_header(h)
            }
        }

        response
    }

    /// Adds a header to the list.
    /// Does all the checks.
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
            match <usize as FromStr>::from_str(header.value.as_str()) {
                Ok(val) => self.data_length = Some(val),
                Err(_) => ()      // wrong value for content-length
            };

            return;
        }

        self.headers.push(header)
    }

    /// Returns the same request, but with an additional header.
    ///
    /// Some headers cannot be modified and some other have a
    ///  special behavior. See the documentation above.
    #[inline]
    pub fn with_header(mut self, header: Header) -> Response<R> {
        self.add_header(header);
        self
    }

    /// Returns the same request, but with a different status code.
    #[inline]
    pub fn with_status_code<S>(mut self, code: S) -> Response<R> where S: Into<StatusCode> {
        self.status_code = code.into();
        self
    }

    /// Returns the same request, but with different data.
    pub fn with_data<S>(self, reader: S, data_length: Option<usize>) -> Response<S> where S: Read {
        Response {
            reader: reader,
            headers: self.headers,
            status_code: self.status_code,
            data_length: data_length,
        }
    }

    /// Prints the HTTP response to a writer.
    ///
    /// This function is the one used to send the response to the client's socket.
    /// Therefore you shouldn't expect anything pretty-printed or even readable.
    ///
    /// The HTTP version and headers passed as arguments are used to
    ///  decide which features (most notably, encoding) to use.
    ///
    /// Note: does not flush the writer.
    pub fn raw_print<W: Write>(mut self, mut writer: W, http_version: HTTPVersion,
                               request_headers: &[Header], do_not_send_body: bool,
                               upgrade: Option<&str>)
                               -> IoResult<()>
    {
        let mut transfer_encoding = Some(choose_transfer_encoding(request_headers,
                                    &http_version, &self.data_length, false /* TODO */));

        // add `Date` if not in the headers
        if self.headers.iter().find(|h| h.field.equiv(&"Date")).is_none() {
            self.headers.insert(0, build_date_header());
        }

        // add `Server` if not in the headers
        if self.headers.iter().find(|h| h.field.equiv(&"Server")).is_none() {
            self.headers.insert(0,
                FromStr::from_str("Server: tiny-http (Rust)").unwrap()
            );
        }

        // handling upgrade
        if upgrade.is_some() {
            let upgrade = upgrade.unwrap();
            self.headers.insert(0, FromStr::from_str(&format!("Upgrade: {}", upgrade)).unwrap());
            self.headers.insert(0, FromStr::from_str("Connection: upgrade").unwrap());
            transfer_encoding = None;
        }

        // checking whether to ignore the body of the response
        let do_not_send_body = do_not_send_body ||
            match self.status_code.as_u16() {
                // sattus code 1xx, 204 and 304 MUST not include a body
                100...199 | 204 | 304 => true,
                _ => false
            };

        // preparing headers for transfer
        match transfer_encoding {
            Some(TransferEncoding::Chunked) => {
                self.headers.push(
                    FromStr::from_str("Transfer-Encoding: chunked").unwrap()
                )
            },

            Some(TransferEncoding::Identity) => {
                assert!(self.data_length.is_some());
                let data_length = self.data_length.unwrap();

                self.headers.push(
                    FromStr::from_str(&format!("Content-Length: {}", data_length)).unwrap()
                )
            },

            _ => ()
        };

        // sending headers
        try!(write_message_header(writer.by_ref(), &http_version,
            &self.status_code, &self.headers));

        // sending the body
        if !do_not_send_body {
            match transfer_encoding {

                Some(TransferEncoding::Chunked) => {
                    use util::ChunksEncoder;

                    let mut writer = ChunksEncoder::new(writer);
                    try!(io::copy(&mut self.reader, &mut writer));
                },

                Some(TransferEncoding::Identity) => {
                    use util::EqualReader;

                    assert!(self.data_length.is_some());
                    let data_length = self.data_length.unwrap();

                    if data_length >= 1 {
                        let (mut equ_reader, _) =
                            EqualReader::new(self.reader.by_ref(), data_length);
                        try!(io::copy(&mut equ_reader, &mut writer));
                    }
                },

                _ => ()

            }
        }

        Ok(())
    }
}

impl Response<File> {
    /// Builds a new `Response` from a `File`.
    ///
    /// The `Content-Type` will **not** be automatically detected,
    ///  you must set it yourself.
    pub fn from_file(file: File) -> Response<File> {
        let file_size = file.metadata().ok().map(|v| v.len() as usize);

        Response::new(
            StatusCode(200),
            Vec::new(),
            file,
            file_size,
            None,
        )
    }
}

impl Response<Cursor<Vec<u8>>> {
    pub fn from_data(data: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            Vec::new(),
            Cursor::new(data),
            Some(data_len),
            None,
        )
    }

    pub fn from_string(data: String) -> Response<Cursor<Vec<u8>>> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            vec!(
                FromStr::from_str("Content-Type: text/plain; charset=UTF-8").unwrap()
            ),
            Cursor::new(data.into_bytes()),
            Some(data_len),
            None,
        )
    }
}

impl Response<io::Empty> {
    /// Builds an empty `Response` with the given status code.

    pub fn new_empty(status_code: StatusCode) -> Response<io::Empty> {
        Response::new(
            status_code,
            Vec::new(),
            io::empty(),
            Some(0),
            None,
        )
    }
}

impl Clone for Response<io::Empty> {
    fn clone(&self) -> Response<io::Empty> {
        Response {
            reader: io::empty(),
            status_code: self.status_code.clone(),
            headers: self.headers.clone(),
            data_length: self.data_length.clone(),
        }
    }
}
