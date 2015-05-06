// Copyright 2015 The tiny-http Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ascii::{AsciiString, AsciiStr};
use std::ascii::AsciiExt;
use std::fmt::{self, Display, Formatter};
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

    pub fn from_u16(in_code: u16) -> StatusCode {
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

impl From<i8> for StatusCode {
    fn from(in_code: i8) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u8> for StatusCode {
    fn from(in_code: u8) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<i16> for StatusCode {
    fn from(in_code: i16) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u16> for StatusCode {
    fn from(in_code: u16) -> StatusCode {
        StatusCode(in_code)
    }
}

impl From<i32> for StatusCode {
    fn from(in_code: i32) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

impl From<u32> for StatusCode {
    fn from(in_code: u32) -> StatusCode {
        StatusCode(in_code as u16)
    }
}

/// Represents a HTTP header.
#[derive(Debug, Clone)]
pub struct Header {
    pub field: HeaderField,
    pub value: AsciiString,
}

impl Header {
    /// Builds a `Header` from two `Vec<u8>`s or two `&[u8]`s.
    ///
    /// Example:
    ///
    /// ```
    /// let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap();
    /// ```
    pub fn from_bytes<B1, B2>(header: B1, value: B2) -> Result<Header, ()>
                              where B1: Into<Vec<u8>> + AsRef<[u8]>,
                                    B2: Into<Vec<u8>> + AsRef<[u8]>
    {
        let header = try!(HeaderField::from_bytes(header).or(Err(())));
        let value = try!(AsciiString::from_bytes(value).or(Err(())));

        Ok(Header { field: header, value: value })
    }

}

impl FromStr for Header {
    type Err = ();

    fn from_str(input: &str) -> Result<Header, ()> {
        let mut elems = input.splitn(2, ':');

        let field = elems.next();
        let value = elems.next();

        let (field, value) = match (field, value) {
            (Some(f), Some(v)) => (f, v),
            _ => return Err(())
        };

        let field = match FromStr::from_str(field) {
            Ok(f) => f,
            _ => return Err(())
        };

        let value = match AsciiStr::from_str(value.trim()) {
            Some(v) => v.to_ascii_string(),
            None => return Err(())
        };

        Ok(Header {
            field: field,
            value: value,
        })
    }
}

impl Display for Header {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "{}: {}", self.field, self.value.as_str())
    }
}

/// Field of a header (eg. `Content-Type`, `Content-Length`, etc.)
///
/// Comparaison between two `HeaderField`s ignores case.
#[derive(Debug, Clone)]
pub struct HeaderField(AsciiString);

impl HeaderField {
    pub fn from_bytes<B>(bytes: B) -> Result<HeaderField, B> where B: Into<Vec<u8>> + AsRef<[u8]> {
        AsciiString::from_bytes(bytes).map(|s| HeaderField(s))
    }

    pub fn as_str<'a>(&'a self) -> &'a AsciiStr {
        match self { &HeaderField(ref s) => s }
    }

    pub fn equiv(&self, other: &'static str) -> bool {
        other.eq_ignore_ascii_case(self.as_str().as_str())
    }
}

impl FromStr for HeaderField {
    type Err = ();

    fn from_str(s: &str) -> Result<HeaderField, ()> {
        AsciiStr::from_str(s.trim()).map(|s| HeaderField(s.to_ascii_string())).ok_or(())
    }
}

impl Display for HeaderField {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let method = self.as_str();
        write!(formatter, "{}", method.as_str())
    }
}

impl PartialEq for HeaderField {
    fn eq(&self, other: &HeaderField) -> bool {
        self.as_str().eq_ignore_ascii_case(other.as_str())
    }
}

impl Eq for HeaderField {}


/// HTTP method (eg. `GET`, `POST`, etc.)
///
/// The user chooses the method he wants.
///
/// Comparaison between two `Method`s ignores case.
#[derive(Debug, Clone)]
pub struct Method(AsciiString);

impl Method {
    fn as_str(&self) -> &AsciiStr {
        match self { &Method(ref s) => s }
    }

    pub fn equiv(&self, other: &'static str) -> bool {
        other.eq_ignore_ascii_case(self.as_str().as_str())
    }
}

impl FromStr for Method {
    type Err = ();

    fn from_str(s: &str) -> Result<Method, ()> {
        <AsciiString as FromStr>::from_str(s).map(|s| Method(s))
    }
}

impl Display for Method {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "{}", self.0)
    }
}

impl PartialEq for Method {
    fn eq(&self, other: &Method) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for Method {}


/// HTTP version (usually 1.0 or 1.1).
#[derive(Debug, Clone, PartialEq, Eq, Ord)]
pub struct HTTPVersion(pub u8, pub u8);

impl Display for HTTPVersion {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), fmt::Error> {
        let (major, minor) = match self { &HTTPVersion(m, n) => (m, n) };
        write!(formatter, "{}.{}", major, minor)
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
        let header: Header = "Content-Type: text/html".parse().unwrap();

        assert!(header.field.equiv(&"content-type"));
        assert!(header.value.as_str() == "text/html");

        assert!("hello world".parse::<Header>().is_err());
    }

    #[test]
    fn test_parse_header_with_doublecolon() {
        let header: Header = "Time: 20: 34".parse().unwrap();

        assert!(header.field.equiv(&"time"));
        assert!(header.value.as_str() == "20: 34");
    }
}
