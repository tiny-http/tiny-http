use crate::connection::Connection;
use crate::util::refined_tcp_stream::Stream as RefinedStream;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// A wrapper around a `native_tls` stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
#[derive(Clone)]
pub(crate) struct NativeTlsStream(Arc<Mutex<native_tls::TlsStream<Connection>>>);

// These struct methods form the implict contract for swappable TLS implementations
impl NativeTlsStream {
    pub(crate) fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .peer_addr()
    }

    pub(crate) fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .shutdown(how)
    }
}

impl Read for NativeTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl Write for NativeTlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .flush()
    }
}

pub(crate) struct NativeTlsContext(native_tls::TlsAcceptor);

impl NativeTlsContext {
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs8(&certificates, &private_key)?;
        let acceptor = native_tls::TlsAcceptor::new(identity)?;
        Ok(Self(acceptor))
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<NativeTlsStream, Box<dyn Error + Send + Sync + 'static>> {
        let stream = self.0.accept(stream)?;
        Ok(NativeTlsStream(Arc::new(Mutex::new(stream))))
    }
}

impl From<NativeTlsStream> for RefinedStream {
    fn from(stream: NativeTlsStream) -> Self {
        RefinedStream::Https(stream)
    }
}
