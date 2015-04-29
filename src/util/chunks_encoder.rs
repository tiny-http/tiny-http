use std::io::Result as IoResult;
use std::io::Write;

/// Splits the incoming data into HTTP chunks.
pub struct ChunksEncoder<W> where W: Write {
    // where to send the result
    output: W,

    // size of each chunk
    chunks_size: usize,

    // data waiting to be sent is stored here
    buffer: Vec<u8>,
}

impl<W> ChunksEncoder<W> where W: Write {
    pub fn new(output: W) -> ChunksEncoder<W> {
        ChunksEncoder::new_with_chunks_size(output, 8192)
    }

    pub fn new_with_chunks_size(output: W, chunks: usize) -> ChunksEncoder<W> {
        ChunksEncoder {
            output: output,
            chunks_size: chunks,
            buffer: Vec::with_capacity(0),
        }
    }
}

fn send<W>(output: &mut W, data: &[u8]) -> IoResult<()> where W: Write {
    try!(write!(output, "{:x}\r\n", data.len()));
    try!(output.write_all(data));
    try!(write!(output, "\r\n"));
    Ok(())
}

impl<W> Write for ChunksEncoder<W> where W: Write {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        try!(self.buffer.write_all(buf));

        while self.buffer.len() >= self.chunks_size {
            let rest = {
                let (to_send, rest) = self.buffer.split_at_mut(self.chunks_size);
                try!(send(&mut self.output, to_send));
                rest.to_vec()
            };
            self.buffer = rest;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.buffer.len() == 0 {
            return Ok(());
        }

        try!(send(&mut self.output, &self.buffer));
        self.buffer.clear();
        Ok(())
    }
}

impl<W> Drop for ChunksEncoder<W> where W: Write {
    fn drop(&mut self) {
        self.flush().ok();
        send(&mut self.output, &[]).ok();
    }
}

#[cfg(test)]
mod test {
    use std::io;
    use std::io::Write;
    use super::ChunksEncoder;
    use ascii::OwnedAsciiCast;

    #[test]
    fn test() {
        let mut source = io::Cursor::new("hello world".to_string().into_bytes());
        let mut dest: Vec<u8> = vec![];

        {
            let mut encoder = ChunksEncoder::new_with_chunks_size(dest.by_ref(), 5);
            io::copy(&mut source, &mut encoder).unwrap();
        }

        let output = dest.into_ascii().unwrap().to_string();

        assert_eq!(output, "5\r\nhello\r\n5\r\n worl\r\n1\r\nd\r\n0\r\n\r\n");
    }
}
