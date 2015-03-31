use std::old_io;
use std::old_io::Reader;
use std::old_io::Writer;
use std::old_io::IoResult;
use std::old_io::net::tcp::TcpStream;
use std::old_io::net::ip::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct ClosableTcpStream {
    stream: TcpStream,
    end_trigger: Arc<AtomicBool>,
    close_read: bool,
    close_write: bool,
    timeout_ms: u32,
}

impl ClosableTcpStream {
    pub fn new(stream: TcpStream, end_trigger: Arc<AtomicBool>,
        close_read: bool, close_write: bool, timeout_ms: u32)
        -> ClosableTcpStream
    {
        ClosableTcpStream {
            stream: stream,
            end_trigger: end_trigger,
            close_read: close_read,
            close_write: close_write,
            timeout_ms: timeout_ms,
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
    /// Reads to this stream is similar to a regular read,
    ///  except that the timeout is predefined.
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        use std::io;
        use time;
        //use std::sync::atomics::Relaxed;

        // getting the time when to stop the loop
        // 10 seconds timeout
        let timeout = time::precise_time_ns()
            + (self.timeout_ms as u64) * 1000 * 1000;

        loop {
            // TODO: this makes some tests fail
            /*if self.end_trigger.load(Relaxed) {
                return Err(old_io::standard_error(old_io::Closed));
            }*/

            self.stream.set_read_timeout(Some(100));

            match self.stream.read(buf) {
                Err(ref err) if err.kind == old_io::TimedOut
                    => (),
                a => return a
            };

            // checking timeout
            if timeout <= time::precise_time_ns() {
                return Err(old_io::standard_error(old_io::TimedOut));
            }
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
                return Err(old_io::standard_error(old_io::Closed));
            }*/

            self.stream.set_write_timeout(Some(100));

            match self.stream.write(buf) {
                Err(ref err) if err.kind == old_io::TimedOut
                    => continue,
                Err(err) => {
                    match err.kind {
                        old_io::ShortWrite(nb) =>
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
