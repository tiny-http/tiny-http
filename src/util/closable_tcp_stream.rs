use std::io::IoResult;
use std::io::net::tcp::TcpStream;
use std::io::net::ip::SocketAddr;
use std::sync::atomics::AtomicBool;
use std::sync::Arc;

pub struct ClosableTcpStream {
    stream: TcpStream,
    end_trigger: Arc<AtomicBool>,
    close_read: bool,
    close_write: bool,
}

impl ClosableTcpStream {
    pub fn new(stream: TcpStream, end_trigger: Arc<AtomicBool>,
               close_read: bool, close_write: bool) -> ClosableTcpStream {

        ClosableTcpStream {
            stream: stream,
            end_trigger: end_trigger,
            close_read: close_read,
            close_write: close_write,
        }
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
        //use std::sync::atomics::Relaxed;

        loop {
            // TODO: this makes some tests fail
            /*if self.end_trigger.load(Relaxed) {
                return Err(io::standard_error(io::Closed));
            }*/

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
        //use std::sync::atomics::Relaxed;

        loop {
            // TODO: this makes some tests fail
            /*if self.end_trigger.load(Relaxed) {
                return Err(io::standard_error(io::Closed));
            }*/

            self.stream.set_write_timeout(Some(100));

            match self.stream.write(buf) {
                Err(ref err) if err.kind == io::TimedOut
                    => continue,
                Err(err) => {
                    match err.kind {
                        io::ShortWrite(nb) =>
                            return self.write(buf.slice_from(nb)),
                        _ => return Err(err)
                    };
                }
                a => return a
            };
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        self.stream.flush()
    }
}
