use std::ascii::{AsciiCast, StrAsciiExt};
use std::fmt::{Formatter, FormatError, Show};

/// Status code of a request or response.
#[deriving(Eq, PartialEq, Clone, Show)]
pub struct StatusCode(pub uint);

impl StatusCode {
    pub fn as_uint(&self) -> uint {
        match *self { StatusCode(n) => n }
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
