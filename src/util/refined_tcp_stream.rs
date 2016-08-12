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

use std::io::{Read, Write};
use std::io::Result as IoResult;
use std::net::{SocketAddr, TcpStream, Shutdown};

#[cfg(feature = "ssl")]
use openssl::ssl::SslStream;

pub struct RefinedTcpStream {
    stream: Stream,
    close_read: bool,
    close_write: bool,
}

pub enum Stream {
    Http(TcpStream),
    #[cfg(feature = "ssl")]
    Https(SslStream<TcpStream>),
}

impl From<TcpStream> for Stream {
    #[inline]
    fn from(stream: TcpStream) -> Stream {
        Stream::Http(stream)
    }
}

#[cfg(feature = "ssl")]
impl From<SslStream<TcpStream>> for Stream {
    #[inline]
    fn from(stream: SslStream<TcpStream>) -> Stream {
        Stream::Https(stream)
    }
}

impl RefinedTcpStream {
    pub fn new<S>(stream: S) -> (RefinedTcpStream, RefinedTcpStream)
        where S: Into<Stream>
    {
        let stream = stream.into();

        let read = match stream {
            Stream::Http(ref stream) => Stream::Http(stream.try_clone().unwrap()),
            #[cfg(feature = "ssl")]
            Stream::Https(ref stream) => Stream::Https(stream.try_clone().unwrap()),
        };

        let read = RefinedTcpStream {
            stream: read,
            close_read: true,
            close_write: false,
        };

        let write = RefinedTcpStream {
            stream: stream,
            close_read: false,
            close_write: true,
        };

        (read, write)
    }

    /// Returns true if this struct wraps arounds a secure connection.
    #[inline]
    pub fn secure(&self) -> bool {
        match self.stream {
            Stream::Http(_) => false,
            #[cfg(feature = "ssl")]
            Stream::Https(_) => true,
        }
    }

    pub fn peer_addr(&mut self) -> IoResult<SocketAddr> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.peer_addr(),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.get_ref().peer_addr(),
        }
    }
}

impl Drop for RefinedTcpStream {
    fn drop(&mut self) {
        if self.close_read {
            match self.stream {
                // ignoring outcome
                Stream::Http(ref mut stream) => stream.shutdown(Shutdown::Read).ok(),
                #[cfg(feature = "ssl")]
                Stream::Https(ref mut stream) => stream.get_mut().shutdown(Shutdown::Read).ok(),
            };
        }

        if self.close_write {
            match self.stream {
                // ignoring outcome
                Stream::Http(ref mut stream) => stream.shutdown(Shutdown::Write).ok(),
                #[cfg(feature = "ssl")]
                Stream::Https(ref mut stream) => stream.get_mut().shutdown(Shutdown::Write).ok(),
            };
        }
    }
}

impl Read for RefinedTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.read(buf),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.read(buf),
        }
    }
}

impl Write for RefinedTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.write(buf),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.flush(),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.flush(),
        }
    }
}
