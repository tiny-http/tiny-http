use std::old_io::{Acceptor, IoResult};
use std::old_io::net::tcp::{TcpAcceptor, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct ClosableTcpAcceptor {
    acceptor: TcpAcceptor,
    end_trigger: Arc<AtomicBool>,
}

impl ClosableTcpAcceptor {
    pub fn new(acceptor: TcpAcceptor, end_trigger: Arc<AtomicBool>) -> ClosableTcpAcceptor {
        ClosableTcpAcceptor {
            acceptor: acceptor,
            end_trigger: end_trigger,
        }
    }
}

impl Acceptor for ClosableTcpAcceptor {
    type Connection = TcpStream;
    fn accept(&mut self) -> IoResult<TcpStream> {
        use std::old_io;
        use std::sync::atomic::Ordering::Relaxed;

        loop {
            if self.end_trigger.load(Relaxed) {
                return Err(old_io::standard_error(old_io::Closed));
            }

            self.acceptor.set_timeout(Some(100));

            match self.acceptor.accept() {
                Err(ref err) if err.kind == old_io::TimedOut
                    => continue,
                a => return a
            };
        }
    }
}
