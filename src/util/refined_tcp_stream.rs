use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};

#[cfg(any(feature = "ssl-openssl", feature = "ssl-rustls"))]
use std::sync::{Arc, Mutex};
#[cfg(feature = "ssl-openssl")]
use openssl::ssl::SslStream;
#[cfg(feature = "ssl")]
use std::sync::{Arc, Mutex};

pub struct RefinedTcpStream {
    stream: Stream,
    close_read: bool,
    close_write: bool,
}

pub enum Stream {
    Http(TcpStream),
    #[cfg(feature = "ssl-openssl")]
    Https(Arc<Mutex<SslStream<TcpStream>>>),
    #[cfg(feature = "ssl-rustls")]
    Https(Arc<Mutex<rustls::StreamOwned<rustls::ServerConnection, TcpStream>>>),
}

impl From<TcpStream> for Stream {
    #[inline]
    fn from(stream: TcpStream) -> Stream {
        Stream::Http(stream)
    }
}

#[cfg(feature = "ssl-openssl")]
impl From<SslStream<TcpStream>> for Stream {
    #[inline]
    fn from(stream: SslStream<TcpStream>) -> Stream {
        Stream::Https(Arc::new(Mutex::new(stream)))
    }
}

#[cfg(feature = "ssl-rustls")]
impl From<rustls::StreamOwned<rustls::ServerConnection, TcpStream>> for Stream {
    #[inline]
    fn from(stream: rustls::StreamOwned<rustls::ServerConnection, TcpStream>) -> Stream {
        Stream::Https(Arc::new(Mutex::new(stream)))
    }
}

impl RefinedTcpStream {
    pub fn new<S>(stream: S) -> (RefinedTcpStream, RefinedTcpStream)
    where
        S: Into<Stream>,
    {
        let stream = stream.into();

        let read = match stream {
            Stream::Http(ref stream) => Stream::Http(stream.try_clone().unwrap()),
            #[cfg(feature = "ssl-openssl")]
            Stream::Https(ref stream) => Stream::Https(Arc::clone(stream)),
            #[cfg(feature = "ssl-rustls")]
            Stream::Https(ref stream) => Stream::Https(Arc::clone(stream)),
        };

        let read = RefinedTcpStream {
            stream: read,
            close_read: true,
            close_write: false,
        };

        let write = RefinedTcpStream {
            stream,
            close_read: false,
            close_write: true,
        };

        (read, write)
    }

    /// Returns true if this struct wraps around a secure connection.
    #[inline]
    pub fn secure(&self) -> bool {
        match self.stream {
            Stream::Http(_) => false,
            #[cfg(any(feature = "ssl-openssl", feature = "ssl-rustls"))]
            Stream::Https(_) => true,
        }
    }

    pub fn peer_addr(&mut self) -> IoResult<SocketAddr> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.peer_addr(),
            #[cfg(feature = "ssl-openssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().get_ref().peer_addr(),
            #[cfg(feature = "ssl-rustls")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().sock.peer_addr(),
        }
    }
}

impl Drop for RefinedTcpStream {
    fn drop(&mut self) {
        if self.close_read {
            match self.stream {
                // ignoring outcome
                Stream::Http(ref mut stream) => stream.shutdown(Shutdown::Read).ok(),
                #[cfg(feature = "ssl-openssl")]
                Stream::Https(ref mut stream) => stream.lock().unwrap().get_mut().shutdown(Shutdown::Read).ok(),
                #[cfg(feature = "ssl-rustls")]
                Stream::Https(ref mut stream) => stream.lock().unwrap().sock.shutdown(Shutdown::Read).ok(),
            };
        }

        if self.close_write {
            match self.stream {
                // ignoring outcome
                Stream::Http(ref mut stream) => stream.shutdown(Shutdown::Write).ok(),
                #[cfg(feature = "ssl-openssl")]
                Stream::Https(ref mut stream) => stream.lock().unwrap().get_mut().shutdown(Shutdown::Write).ok(),
                #[cfg(feature = "ssl-rustls")]
                Stream::Https(ref mut stream) => stream.lock().unwrap().sock.shutdown(Shutdown::Write).ok(),
            };
        }
    }
}

impl Read for RefinedTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.read(buf),
            #[cfg(feature = "ssl-openssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().read(buf),
            #[cfg(feature = "ssl-rustls")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().read(buf),
        }
    }
}

impl Write for RefinedTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.write(buf),
            #[cfg(feature = "ssl-openssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().write(buf),
            #[cfg(feature = "ssl-rustls")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().write(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.flush(),
            #[cfg(feature = "ssl-openssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().flush(),
            #[cfg(feature = "ssl-rustls")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().flush(),
        }
    }
}
