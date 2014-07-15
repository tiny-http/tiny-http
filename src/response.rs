use common::{Header, HTTPVersion, StatusCode};
use std::io::{IoResult, MemReader};
use std::io::fs::File;
use std::io::util;
use std::io::util::NullReader;
use chunks::ChunksEncoder;

/// Object representing an HTTP response.
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
///      been read, `0`s will be sent.
#[experimental]
pub struct Response<R> {
    reader: R,
    status_code: StatusCode,
    headers: Vec<Header>,
    http_version: HTTPVersion,
}

impl<R: Reader> Response<R> {
    #[experimental]
    pub fn new(status_code: StatusCode, mut headers: Vec<Header>,
               data: R, data_length: Option<uint>) -> Response<R>
    {
        // add Content-Length if not in the headers
        if data_length.is_some() {
            if headers.iter().find(|h| h.field.equiv(&"Content-Length")).is_none() {
                headers.unshift(
                    Header{
                        field: from_str("Content-Length").unwrap(),
                        value: format!("{}", data_length.unwrap())
                    }
                );
            }
        }

        Response {
            reader: data,
            status_code: status_code,
            headers: headers,
            http_version: HTTPVersion(1, 1),
        }
    }

    /// Returns the same request, but with an additional header.
    #[experimental]
    pub fn with_header(mut self, header: Header) -> Response<R> {
        self.headers.push(header);
        self
    }

    /// Returns the same request, but with a different status code.
    #[experimental]
    pub fn with_status_code(mut self, code: StatusCode) -> Response<R> {
        self.status_code = code;
        self
    }

    /// Forces an HTTP version.
    #[experimental]
    pub fn with_http_version(mut self, version: HTTPVersion) -> Response<R> {
        self.http_version = version;
        self
    }

    /// Cleans-up the headers so that they can be returned.
    #[experimental]
    fn purify_headers(&mut self) {
        // removing some unwanted headers, like Connection
        self.headers.retain(|h| !h.field.equiv(&"Connection"));

        // add Server if not in the headers
        if self.headers.iter().find(|h| h.field.equiv(&"Server")).is_none() {
            self.headers.unshift(
                Header{field: from_str("Server").unwrap(), value: "tiny-http (Rust)".to_string()}
            );
        }

        // add Connection: close
        /*self.headers.push(
            Header{field: "Connection".to_string(), value: "close".to_string()}
        );*/
    }

    /// Prints the HTTP response to a writer.
    #[experimental]
    pub fn raw_print<W: Writer>(mut self, mut writer: W) -> IoResult<()> {
        self.purify_headers();

        // if we don't have a Content-Length, or if the Content-Length is too big, using chunks writer
        let chunks_threshold = 32768;
        let use_chunks = 
            self.http_version >= HTTPVersion(1, 1) &&
            self.headers.iter().find(|h| h.field.equiv(&"Content-Length"))
                .and_then(|h| from_str::<uint>(h.value.as_slice()))
                .filtered(|val| *val < chunks_threshold)
                .is_none();

        // add transfer-encoding header
        if use_chunks {
            self.headers.push(
                Header{field: from_str("Transfer-Encoding").unwrap(), value: "chunked".to_string()}
            )
        }

        // writing status line
        try!(write!(writer, "HTTP/{} {} {}\r\n",
            self.http_version,
            self.status_code.as_uint(),
            self.status_code.get_default_reason_phrase()
        ));

        // writing headers
        for header in self.headers.iter() {
            try!(write!(writer, "{}: {}\r\n", header.field, header.value));
        }

        // separator between header and data
        try!(write!(writer, "\r\n"));

        // writing data
        if use_chunks {
            let mut writer = ChunksEncoder::new(writer);
            try!(util::copy(&mut self.reader, &mut writer));
        } else {
            try!(util::copy(&mut self.reader, &mut writer));
        }

        Ok(())
    }
}

impl Response<File> {
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
    #[experimental]
    pub fn empty() -> Response<NullReader> {
        Response::new(
            StatusCode(204),
            Vec::new(),
            NullReader,
            Some(0)
        )
    }

    #[experimental]
    pub fn not_modified() -> Response<NullReader> {
        Response::new(
            StatusCode(304),
            Vec::new(),
            NullReader,
            Some(0)
        )
    }
}
