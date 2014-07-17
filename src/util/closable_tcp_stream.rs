use std::io::IoResult;
use std::io::net::tcp::TcpStream;
use std::io::net::ip::SocketAddr;

pub struct ClosableTcpStream {
    stream: TcpStream,
    close: Receiver<()>,
    close_read: bool,
    close_write: bool,
}

impl ClosableTcpStream {
    pub fn new(stream: TcpStream, close_read: bool, close_write: bool) -> (ClosableTcpStream, Sender<()>) {
        let (tx, rx) = channel();

        let acc = ClosableTcpStream {
            stream: stream,
            close: rx,
            close_read: close_read,
            close_write: close_write,
        };

        (acc, tx)
    }

    pub fn peer_name(&mut self) -> IoResult<SocketAddr> {
        self.stream.peer_name()
    }
}

impl Drop for ClosableTcpStream {
    fn drop(&mut self) {
        if self.close_read {
            self.stream.close_read().ok();      // ignoring outcome
        }
        if self.close_write {
            self.stream.close_write().ok();     // ignoring outcome
        }
    }
}

impl Reader for ClosableTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        use std::io;

        loop {
            if self.close.try_recv().is_ok() {
                return Err(io::standard_error(io::Closed));
            }

            self.stream.set_read_timeout(Some(100));

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

            self.stream.set_write_timeout(Some(100));

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
