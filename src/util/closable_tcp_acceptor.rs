use std::io::{Acceptor, IoResult};
use std::io::net::tcp::{TcpAcceptor, TcpStream};
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

impl Acceptor<TcpStream> for ClosableTcpAcceptor {
    fn accept(&mut self) -> IoResult<TcpStream> {
        use std::io;
        use std::sync::atomic::Relaxed;

        loop {
            if self.end_trigger.load(Relaxed) {
                return Err(io::standard_error(io::Closed));
            }

            self.acceptor.set_timeout(Some(100));

            match self.acceptor.accept() {
                Err(ref err) if err.kind == io::TimedOut
                    => continue,
                a => return a
            };
        }
    }
}
