use std::io::IoResult;

/// Reads HTTP chunks and sends back real data.
pub struct ChunksDecoder<R> {
    // where the chunks come from
    source: R,

    // remaining size of the chunk being read
    // none if we are not in a chunk
    remaining_chunks_size: Option<uint>,

    // data from the start of the current chunk
    buffer: Vec<u8>,
}

impl<R: Reader> ChunksDecoder<R> {
    pub fn new(source: R) -> ChunksDecoder<R> {
        ChunksDecoder {
            source: source,
            remaining_chunks_size: None,
            buffer: Vec::new(),
        }
    }
}

impl<R: Reader> Reader for ChunksDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        use std::io;
        use std::path::BytesContainer;

        // first possibility: we are not in a chunk
        if self.remaining_chunks_size.is_none() {
            use std::num::FromStrRadix;

            // trying the read the chunk size
            let mut chunk_size = Vec::new();

            loop {
                let byte = try!(self.source.read_byte());

                if byte == b'\r' {
                    break
                }

                chunk_size.push(byte);
            }

            if try!(self.source.read_byte()) != b'\n' {
                return Err(io::standard_error(io::InvalidInput));
            }

            let chunk_size = match chunk_size.container_as_str() {
                Some(c) => c,
                None => return Err(io::standard_error(io::InvalidInput))
            };

            let chunk_size: uint = match FromStrRadix::from_str_radix(chunk_size, 16) {
                Some(c) => c,
                None => return Err(io::standard_error(io::InvalidInput))
            };

            // if the chunk size is 0, we are at EOF
            if chunk_size == 0 {
                if try!(self.source.read_byte()) != b'\r' {
                    return Err(io::standard_error(io::InvalidInput));
                }
                if try!(self.source.read_byte()) != b'\n' {
                    return Err(io::standard_error(io::InvalidInput));
                }
                return Err(io::standard_error(io::EndOfFile));
            }

            // now that we now the current chunk size, calling ourselves recursively
            self.remaining_chunks_size = Some(chunk_size);
            return self.read(buf)
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

            if try!(self.source.read_byte()) != b'\r' {
                return Err(io::standard_error(io::InvalidInput));
            }
            if try!(self.source.read_byte()) != b'\n' {
                return Err(io::standard_error(io::InvalidInput));
            }
        }

        return Ok(read);
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use super::ChunksDecoder;

    #[test]
    fn test() {
        let source = io::MemReader::new("3\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        let decoded = decoded.read_to_string().unwrap();

        assert_eq!(decoded.as_slice(), "hello world!!!");
    }

    #[test]
    #[should_fail]
    fn invalid_input1() {
        let source = io::MemReader::new("2\r\nhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        decoded.read_to_string().unwrap();
    }

    #[test]
    #[should_fail]
    fn invalid_input2() {
        let source = io::MemReader::new("3\rhel\r\nb\r\nlo world!!!\r\n0\r\n".to_string().into_bytes());
        let mut decoded = ChunksDecoder::new(source);

        decoded.read_to_string().unwrap();
    }
}
