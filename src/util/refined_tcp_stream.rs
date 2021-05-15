use std::fmt;
use std::io::Result as IoResult;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr as TcpAddr, TcpStream};
use std::os::unix::net::{SocketAddr as UnixAddr, UnixStream};

#[cfg(feature = "ssl")]
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
    #[cfg(feature = "ssl")]
    Https(Arc<Mutex<SslStream<TcpStream>>>),
    Unix(UnixStream),
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
        Stream::Https(Arc::new(Mutex::new(stream)))
    }
}

impl From<UnixStream> for Stream {
    #[inline]
    fn from(stream: UnixStream) -> Stream {
        Stream::Unix(stream)
    }
}

#[derive(Clone)]
pub enum PeerAddr {
    Tcp(TcpAddr),
    Unix(UnixAddr),
}

impl From<TcpAddr> for PeerAddr {
    #[inline]
    fn from(addr: TcpAddr) -> PeerAddr {
        PeerAddr::Tcp(addr)
    }
}

impl From<UnixAddr> for PeerAddr {
    #[inline]
    fn from(addr: UnixAddr) -> PeerAddr {
        PeerAddr::Unix(addr)
    }
}

impl fmt::Display for PeerAddr {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            PeerAddr::Tcp(addr) => write!(formatter, "TCP {}", addr),
            PeerAddr::Unix(addr) => write!(formatter, "Unix {:?}", addr),
        }
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
            #[cfg(feature = "ssl")]
            Stream::Https(ref stream) => Stream::Https(stream.clone()),
            Stream::Unix(ref stream) => Stream::Unix(stream.try_clone().unwrap()),
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

    /// Returns true if this struct wraps arounds a secure connection.
    #[inline]
    pub fn secure(&self) -> bool {
        match self.stream {
            Stream::Http(_) => false,
            #[cfg(feature = "ssl")]
            Stream::Https(_) => true,
            Stream::Unix(_) => false,
        }
    }

    pub fn peer_addr(&mut self) -> IoResult<PeerAddr> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.peer_addr().map(Into::into),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => {
                stream.lock().unwrap().get_ref().peer_addr().map(Into::into)
            }
            Stream::Unix(ref mut stream) => stream.peer_addr().map(Into::into),
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
                Stream::Https(ref mut stream) => stream
                    .lock()
                    .unwrap()
                    .get_mut()
                    .shutdown(Shutdown::Read)
                    .ok(),
                Stream::Unix(ref mut stream) => stream.shutdown(Shutdown::Read).ok(),
            };
        }

        if self.close_write {
            match self.stream {
                // ignoring outcome
                Stream::Http(ref mut stream) => stream.shutdown(Shutdown::Write).ok(),
                #[cfg(feature = "ssl")]
                Stream::Https(ref mut stream) => stream
                    .lock()
                    .unwrap()
                    .get_mut()
                    .shutdown(Shutdown::Write)
                    .ok(),
                Stream::Unix(ref mut stream) => stream.shutdown(Shutdown::Write).ok(),
            };
        }
    }
}

impl Read for RefinedTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.read(buf),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().read(buf),
            Stream::Unix(ref mut stream) => stream.read(buf),
        }
    }
}

impl Write for RefinedTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.write(buf),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().write(buf),
            Stream::Unix(ref mut stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.stream {
            Stream::Http(ref mut stream) => stream.flush(),
            #[cfg(feature = "ssl")]
            Stream::Https(ref mut stream) => stream.lock().unwrap().flush(),
            Stream::Unix(ref mut stream) => stream.flush(),
        }
    }
}
