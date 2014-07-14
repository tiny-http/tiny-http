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
    pub field: String,
    pub value: String,
}

impl Show for Header {
    fn fmt(&self, formatter: &mut Formatter) -> Result<(), FormatError> {
        (format!("{}: {}", self.field, self.value)).fmt(formatter)
    }
}

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
