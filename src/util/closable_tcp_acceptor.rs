use std::io::{Acceptor, IoResult};
use std::io::net::tcp::{TcpAcceptor, TcpStream};

pub struct ClosableTcpAcceptor {
    acceptor: TcpAcceptor,
    close: Receiver<()>,
}

impl ClosableTcpAcceptor {
    pub fn new(mut acceptor: TcpAcceptor) -> (ClosableTcpAcceptor, Sender<()>) {
        let (tx, rx) = channel();

        let acc = ClosableTcpAcceptor {
            acceptor: acceptor,
            close: rx,
        };

        (acc, tx)
    }
}

impl Acceptor<TcpStream> for ClosableTcpAcceptor {
    fn accept(&mut self) -> IoResult<TcpStream> {
        use std::io;

        loop {
            if self.close.try_recv().is_ok() {
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
