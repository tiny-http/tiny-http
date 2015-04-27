use std::fmt::Debug;
use ascii::{Ascii, AsciiCast};
use std::fmt::{self, Formatter};
use std::str::{FromStr};
use std::cmp::Ordering;

/// Status code of a request or response.
#[derive(Eq, PartialEq, Clone, Debug, Ord, PartialOrd)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// Returns the status code as a number.
    pub fn as_u16(&self) -> u16 {
        match *self { StatusCode(n) => n }
    }

    pub fn from_usize(in_code: usize) -> StatusCode {
        StatusCode(in_code)
    }

    /// Returns the default reason phrase for this status code.
    /// For example the status code 404 corresponds to "Not Found".
    pub fn get_default_reason_phrase(&self) -> &'static str {
        match self.as_u16() {
            100 => "Continue",
            101 => "Switching Protocols",
            102 => "Processing",
            118 => "Connection timed out",
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            203 => "Non-Authoritative Information",
            204 => "No Content",
            205 => "Reset Content",
            206 => "Partial Content",
            207 => "Multi-Status",
            210 => "Content Different",
            300 => "Multiple Choices",
            301 => "Moved Permanently",
            302 => "Found",
            303 => "See Other",
            304 => "Not Modified",
            305 => "Use Proxy",
            307 => "Temporary Redirect",
            400 => "Bad Request",
            401 => "Unauthorized",
            402 => "Payment Required",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            406 => "Not Acceptable",
            407 => "Proxy Authentication Required",
            408 => "Request Time-out",
            409 => "Conflict",
            410 => "Gone",
            411 => "Length Required",
            412 => "Precondition Failed",
            413 => "Request Entity Too Large",
            414 => "Reques-URI Too Large",
            415 => "Unsupported Media Type",
            416 => "Request range not satisfiable",
            417 => "Expectation Failed",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Time-out",
            505 => "HTTP Version not supported",
            _ => "Unknown"
        }
    }

    pub fn equiv(&self, other: &u16) -> bool {
        self.as_u16() == *other
    }
}

/// Represents a HTTP header.
/// 
/// The easiest way to create a `Header` object is to call `from_str`.
/// 
/// ```
/// let header: tiny_http::Header = from_str("Content-Type: text/plain").unwrap();
/// ```
#[derive(Clone)]
pub struct Header {
    pub field: HeaderField,
    pub value: Vec<Ascii>,
}

impl FromStr for Header {
    type Err = ();

    fn from_str(input: &str) -> Option<Header> {
        let mut elems = input.splitn(1, ':');

        let field = elems.next();
        let value = elems.next();

        let (field, value) = match (field, value) {
            (Some(f), Some(v)) => (f, v),
            _ => return None
        };

        let field = match FromStr::from_str(field) {
            Ok(f) => f,
            _ => return None
        };

        let value = match value.trim().to_ascii_opt() {
            Some(v) => v.to_vec(),
            None => return None
        };

        Some(Header {
            field: field,
            value: value,
        })
    }
}

impl Debug for Header {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let value = self.value.as_slice();
        let value = value.as_str_ascii();
        (format!("{}: {}", self.field, value)).fmt(formatter)
    }
}

/// Field of a header (eg. `Content-Type`, `Content-Length`, etc.)
/// 
/// Comparaison between two `HeaderField`s ignores case.
#[derive(Clone)]
pub struct HeaderField(Vec<Ascii>);

impl HeaderField {
    pub fn as_str<'a>(&'a self) -> &'a [Ascii] {
        match self { &HeaderField(ref s) => s.as_slice() }
    }

    pub fn equiv(&self, other: &'static str) -> bool {
        other.as_slice().eq_ignore_ascii_case(self.as_str().as_str_ascii())
    }
}

impl FromStr for HeaderField {
    type Err = ();

    fn from_str(s: &str) -> Option<HeaderField> {
        s.trim().to_ascii_opt().map(|s| HeaderField(s.to_vec()))
    }
}

impl ToString for HeaderField {
    fn to_string(self) -> String {
        match self { HeaderField(s) => s.to_string() }
    }
}

impl Debug for HeaderField {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let method = self.as_str();
        method.as_str_ascii().fmt(formatter)
    }
}

impl PartialEq for HeaderField {
    fn eq(&self, other: &HeaderField) -> bool {
        self.as_str().eq_ignore_case(other.as_str())
    }
}

impl Eq for HeaderField {}


/// HTTP method (eg. `GET`, `POST`, etc.)
/// 
/// The user chooses the method he wants.
/// 
/// Comparaison between two `Method`s ignores case.

#[derive(Clone)]
pub struct Method(Vec<Ascii>);

impl Method {
    fn as_str<'a>(&'a self) -> &'a [Ascii] {
        match self { &Method(ref s) => s.as_slice() }
    }

    pub fn equiv(&self, other: &'static str) -> bool {
        other.as_slice().eq_ignore_ascii_case(self.as_str().as_str_ascii())
    }
}

impl FromStr for Method {
    type Err = ();

    fn from_str(s: &str) -> Option<Method> {
        s.to_ascii_opt().map(|s| Method(s.to_vec()))
    }
}

impl ToString for Method {
    fn to_string(self) -> String {
        match self { Method(s) => s.to_string() }
    }
}

impl Debug for Method {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let method = self.as_str();
        method.as_str_ascii().fmt(formatter)
    }
}

impl PartialEq for Method {
    fn eq(&self, other: &Method) -> bool {
        self.as_str().eq_ignore_case(other.as_str())
    }
}

impl Eq for Method {}


/// HTTP version (usually 1.0 or 1.1).

#[derive(Clone, PartialEq, Eq, Ord)]
pub struct HTTPVersion(pub usize, pub usize);

impl Debug for HTTPVersion {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let (major, minor) = match self { &HTTPVersion(m, n) => (m, n) };
        (format!("{}.{}", major, minor)).fmt(formatter)
    }
}

impl PartialOrd for HTTPVersion {
    fn partial_cmp(&self, other: &HTTPVersion) -> Option<Ordering> {
        let (my_major, my_minor) = match self { &HTTPVersion(m, n) => (m, n) };
        let (other_major, other_minor) = match other { &HTTPVersion(m, n) => (m, n) };

        if my_major != other_major {
            return my_major.partial_cmp(&other_major)
        }

        my_minor.partial_cmp(&other_minor)
    }
}


#[cfg(test)]
mod test {
    use super::Header;

    #[test]
    fn test_parse_header() {
        use ascii::AsciiStr;

        let header: Header = FromStr::from_str("Content-Type: text/html").unwrap();

        assert!(header.field.equiv(&"content-type"));
        assert!(header.value.as_slice().as_str_ascii() == "text/html");

        assert!(FromStr::from_str::<Header>("hello world").is_none());
    }

    #[test]
    fn test_parse_header_with_doublecolon() {
        use ascii::AsciiStr;

        let header: Header = FromStr::from_str("Time: 20: 34").unwrap();

        assert!(header.field.equiv(&"time"));
        assert!(header.value.as_slice().as_str_ascii() == "20: 34");
    }
}
