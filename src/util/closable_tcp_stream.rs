use std::io::{Acceptor, IoResult};
use std::io::net::tcp::TcpStream;
use std::io::net::ip::SocketAddr;

pub struct ClosableTcpStream {
    stream: TcpStream,
    close: Receiver<()>,
}

impl ClosableTcpStream {
    pub fn new(mut stream: TcpStream) -> (ClosableTcpStream, Sender<()>) {
        let (tx, rx) = channel();

        stream.set_timeout(Some(100));

        let acc = ClosableTcpStream {
            stream: stream,
            close: rx,
        };

        (acc, tx)
    }

    pub fn peer_name(&mut self) -> IoResult<SocketAddr> {
        self.stream.peer_name()
    }
}

impl Drop for ClosableTcpStream {
    fn drop(&mut self) {
        self.stream.close_read().ok();      // ignoring outcome
        self.stream.close_write().ok();     // ignoring outcome
    }
}

impl Reader for ClosableTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        use std::io;

        loop {
            if self.close.try_recv().is_ok() {
                return Err(io::standard_error(io::Closed));
            }

            match self.stream.read(buf) {
                Err(ref err) if err.kind == io::TimedOut
                    => continue,
                a => return a
            };
        }
    }
}

impl Writer for ClosableTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        use std::io;

        loop {
            if self.close.try_recv().is_ok() {
                return Err(io::standard_error(io::Closed));
            }

            match self.stream.write(buf) {
                Err(ref err) if err.kind == io::TimedOut
                    => continue,
                Err(err) => {
                    match err.kind {
                        io::ShortWrite(nb) => return self.write(buf.slice_from(nb)),
                        _ => return Err(err)
                    };
                }
                a => return a
            };
        }
    }
}
