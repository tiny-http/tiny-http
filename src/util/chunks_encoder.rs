use std::io::IoResult;

/// Splits the incoming data into HTTP chunks.
pub struct ChunksEncoder<W> {
    // where to send the result
    output: W,

    // size of each chunk
    chunks_size: uint,

    // data waiting to be sent is stored here
    buffer: Vec<u8>,
}

impl<W: Writer> ChunksEncoder<W> {
    pub fn new(output: W) -> ChunksEncoder<W> {
        ChunksEncoder::new_with_chunks_size(output, 8192)
    }

    pub fn new_with_chunks_size(output: W, chunks: uint) -> ChunksEncoder<W> {
        ChunksEncoder {
            output: output,
            chunks_size: chunks,
            buffer: Vec::new(),
        }
    }
}

fn send<W: Writer>(output: &mut W, data: &[u8]) -> IoResult<()> {
    try!(write!(output, "{:x}\r\n", data.len()));
    try!(output.write(data));
    try!(write!(output, "\r\n"));
    Ok(())
}

impl<W: Writer> Writer for ChunksEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.buffer.push_all(buf);

        while self.buffer.len() >= self.chunks_size {
            let rest = {
                let (to_send, rest) = self.buffer.split_at_mut(self.chunks_size);
                try!(send(&mut self.output, to_send));
                rest.to_vec()
            };
            self.buffer = rest;
        }

        Ok(())
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.buffer.len() == 0 {
            return Ok(());
        }

        try!(send(&mut self.output, self.buffer.as_slice()));
        self.buffer.clear();
        Ok(())
    }
}

#[unsafe_destructor]
impl<W: Writer> Drop for ChunksEncoder<W> {
    fn drop(&mut self) {
        self.flush().ok();
        send(&mut self.output, []).ok();
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use super::ChunksEncoder;

    #[test]
    fn test() {
        let mut source = io::MemReader::new("hello world".to_string().into_bytes());
        let mut dest = io::MemWriter::new();

        {
            let mut encoder = ChunksEncoder::new_with_chunks_size(dest.by_ref(), 5);
            io::util::copy(&mut source, &mut encoder).unwrap();
        }

        let output = dest.unwrap().into_ascii().into_string();

        assert_eq!(output.as_slice(), "5\r\nhello\r\n5\r\n worl\r\n1\r\nd\r\n0\r\n\r\n");
    }
}
