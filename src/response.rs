use common::{Header, HTTPVersion, StatusCode};
use std::io::{IoResult, MemReader};
use std::io::fs::File;
use std::io::util;
use std::io::util::NullReader;
use chunks::ChunksEncoder;

/// Object representing an HTTP response.
pub struct Response<R> {
    reader: R,
    status_code: StatusCode,
    headers: Vec<Header>,
    http_version: HTTPVersion,
}

impl<R: Reader> Response<R> {
    pub fn new(status_code: StatusCode, mut headers: Vec<Header>,
               data: R, data_length: uint) -> Response<R>
    {
        // add Content-Length if not in the headers
        if headers.iter().find(|h| h.field.equiv(&"Content-Length")).is_none() {
            headers.unshift(
                Header{field: from_str("Content-Length").unwrap(), value: format!("{}", data_length)}
            );
        }

        Response {
            reader: data,
            status_code: status_code,
            headers: headers,
            http_version: HTTPVersion(1, 1),
        }
    }

    /// Returns the same request, but with an additional header.
    pub fn with_header(mut self, header: Header) -> Response<R> {
        self.headers.push(header);
        self
    }

    /// Returns the same request, but with a different status code.
    pub fn with_status_code(mut self, code: StatusCode) -> Response<R> {
        self.status_code = code;
        self
    }

    /// Forces an HTTP version.
    pub fn with_http_version(mut self, version: HTTPVersion) -> Response<R> {
        self.http_version = version;
        self
    }

    /// Cleans-up the headers so that they can be returned.
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
            self.status_code.get_default_message()
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
    pub fn from_file(file: &Path) -> IoResult<Response<File>> {
        let mut file = try!(File::open(file));
        let stats = try!(file.stat());

        Ok(Response::new(
            StatusCode(200),
            Vec::new(),
            file,
            stats.size as uint
        ))
    }
}

impl Response<MemReader> {
    pub fn from_data(data: Vec<u8>) -> Response<MemReader> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            Vec::new(),
            MemReader::new(data),
            data_len
        )
    }

    pub fn from_string(data: String) -> Response<MemReader> {
        let data_len = data.len();

        Response::new(
            StatusCode(200),
            vec!(
                from_str("Content-Type: text/plain; charset=UTF-8").unwrap()
            ),
            MemReader::new(data.into_bytes()),
            data_len
        )        
    }
}

impl Response<NullReader> {
    pub fn empty() -> Response<NullReader> {
        Response::new(
            StatusCode(204),
            Vec::new(),
            NullReader,
            0
        )
    }

    pub fn not_modified() -> Response<NullReader> {
        Response::new(
            StatusCode(304),
            Vec::new(),
            NullReader,
            0
        )
    }
}
