use std::io::Result as IoResult;
use std::io::Read;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::error::Error;

/// Reads HTTP chunks and sends back real data.
pub struct ChunksDecoder<R> {
    // where the chunks come from
    source: R,

    // remaining size of the chunk being read
    // none if we are not in a chunk
    remaining_chunks_size: Option<usize>,

    // data from the start of the current chunk
    buffer: Vec<u8>,
}

pub struct ChunksError;

impl Error for ChunksError {
    fn description(&self) -> &str {
        "Error while decoding chunks"
    }
}

impl<R> ChunksDecoder<R> where R: Read {
    pub fn new(source: R) -> ChunksDecoder<R> {
        ChunksDecoder {
            source: source,
            remaining_chunks_size: None,
            buffer: Vec::with_capacity(128),
        }
    }
}

impl<R> Read for ChunksDecoder<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // first possibility: we are not in a chunk
        if self.remaining_chunks_size.is_none() {
            // trying the read the chunk size
            let mut chunk_size = Vec::new();

            loop {
                let byte = try!(self.source.by_ref().bytes().next());
                if byte == b'\r' {
                    break;
                }

                chunk_size.push(byte);
            }

            if try!(self.source.by_ref().bytes().next()) != b'\n' {
                return Err(IoError::new(ErrorKind::InvalidInput, ChunksError));
            }

            let chunk_size = match chunk_size.container_as_str() {
                Some(c) => c,
                None => return Err(IoError::new(ErrorKind::InvalidInput, ChunksError))
            };

            let chunk_size: usize = match FromStrRadix::from_str_radix(chunk_size, 16) {
                Ok(c) => c,
                Err(()) => return Err(IoError::new(ErrorKind::InvalidInput, ChunksError))
            };

            // if the chunk size is 0, we are at EOF
            if chunk_size == 0 {
                if try!(self.source.by_ref().bytes().next()) != b'\r' {
                    return Err(IoError::new(ErrorKind::InvalidInput, ChunksError));
                }
                if try!(self.source.by_ref().bytes().next()) != b'\n' {
                    return Err(IoError::new(ErrorKind::InvalidInput, ChunksError));
                }
                return Ok(0);
            }

            // now that we now the current chunk size, calling ourselves recursively
            self.remaining_chunks_size = Some(chunk_size);
            return self.read(buf);
        }

        assert!(self.remaining_chunks_size.is_some());

        // second possibility: we continue reading from a chunk
        if buf.len() < *self.remaining_chunks_size.as_ref().unwrap() {
            let read = try!(self.source.read(buf));
            *self.remaining_chunks_size.as_mut().unwrap() -= read;
            return Ok(read);
        }

        // third possibility: the read request goes further than the current chunk
        // we simply read until the end of the chunk and return
        assert!(buf.len() >= *self.remaining_chunks_size.as_ref().unwrap());

        let remaining_chunks_size = *self.remaining_chunks_size.as_ref().unwrap();

        let buf = buf.slice_to_mut(remaining_chunks_size);
        let read = try!(self.source.read(buf));
        *self.remaining_chunks_size.as_mut().unwrap() -= read;

        if read == remaining_chunks_size {
            self.remaining_chunks_size = None;

            if try!(self.source.by_ref().bytes().next()) != b'\r' {
                return Err(IoError::new(ErrorKind::InvalidInput, ChunksError));
            }
            if try!(self.source.by_ref().bytes().next()) != b'\n' {
                return Err(IoError::new(ErrorKind::InvalidInput, ChunksError));
            }
        }

        return Ok(read);
    }
}

#[cfg(test)]
mod test {
    use std::old_io;
    use super::ChunksDecoder;

    #[test]
    fn test() {
        let source = io::Cursor::new("3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        let decoded = decoded.read_to_string().unwrap();

        assert_eq!(decoded.as_slice(), "hello world!!!");
    }

    #[test]
    #[should_panic]
    fn invalid_input1() {
        let source = io::Cursor::new("2\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        decoded.read_to_string().unwrap();
    }

    #[test]
    #[should_panic]
    fn invalid_input2() {
        let source = io::Cursor::new("3\rhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        decoded.read_to_string().unwrap();
    }
}
