use std::ascii::{AsciiCast, StrAsciiExt};
use std::fmt::{Formatter, FormatError, Show};

/// Status code of a request or response.
#[deriving(Eq, PartialEq, Clone, Show)]
pub struct StatusCode(pub uint);

impl StatusCode {
    pub fn as_uint(&self) -> uint {
        match *self { StatusCode(n) => n }
    }

    pub fn get_default_message(&self) -> &'static str {
        match self.as_uint() {
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
            400 => "Bad Request",
            401 => "Unauthorized",
            402 => "Payment Required",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            406 => "Not Acceptable",
            // TODO: finish
            _ => "Unknown"
        }
    }
}

#[deriving(Clone)]
pub struct Header {
    pub field: HeaderField,
    pub value: String,
}

impl Show for Header {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), FormatError> {
        (format!("{}: {}", self.field, self.value)).fmt(formatter)
    }
}

/// Field of a header.
/// eg. Content-Type, Content-Length, etc.
/// Comparaison between two HeaderFields ignores case.
#[deriving(Clone)]
pub struct HeaderField(Vec<Ascii>);

impl HeaderField {
    fn as_str<'a>(&'a self) -> &'a [Ascii] {
        match self { &HeaderField(ref s) => s.as_slice() }
    }
}

impl ::std::from_str::FromStr for HeaderField {
    fn from_str(s: &str) -> Option<HeaderField> {
        s.to_ascii_opt().map(|s| HeaderField(Vec::from_slice(s)))
    }
}

impl IntoStr for HeaderField {
    fn into_string(self) -> String {
        match self { HeaderField(s) => s.into_string() }
    }
}

impl Show for HeaderField {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), FormatError> {
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

impl<S: Str> Equiv<S> for HeaderField {
    fn equiv(&self, other: &S) -> bool {
        other.as_slice().eq_ignore_ascii_case(self.as_str().as_str_ascii())
    }
}


/// HTTP method (eg. GET, POST, etc.)
/// The user chooses the method he wants.
/// Comparaison between two HeaderFields ignores case.
#[deriving(Clone)]
pub struct Method(Vec<Ascii>);

impl Method {
    fn as_str<'a>(&'a self) -> &'a [Ascii] {
        match self { &Method(ref s) => s.as_slice() }
    }
}

impl ::std::from_str::FromStr for Method {
    fn from_str(s: &str) -> Option<Method> {
        s.to_ascii_opt().map(|s| Method(Vec::from_slice(s)))
    }
}

impl IntoStr for Method {
    fn into_string(self) -> String {
        match self { Method(s) => s.into_string() }
    }
}

impl Show for Method {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), FormatError> {
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

impl<S: Str> Equiv<S> for Method {
    fn equiv(&self, other: &S) -> bool {
        other.as_slice().eq_ignore_ascii_case(self.as_str().as_str_ascii())
    }
}

/// HTTP version (usually 1.0 or 1.1).
#[deriving(Clone, PartialEq, Eq, Ord)]
pub struct HTTPVersion(pub uint, pub uint);

impl Show for HTTPVersion {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), FormatError> {
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
