use std::io::{Read, Write, Result as IoResult};

use crate::ReadWrite;

// Example usage with CustomStream
pub struct CustomStream<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> CustomStream<R, W>
where
    R: Read,
    W: Write,
{
    pub fn new(reader: R, writer: W) -> CustomStream<R, W> {
        CustomStream { reader, writer }
    }
}

impl<R, W> ReadWrite for CustomStream<R, W>
where
    R: Read,
    W: Write,
{
    fn reader(&self) -> &dyn Read {
        &self.reader
    }

    fn writer(&self) -> &dyn Write {
        &self.writer
    }
}

// Implement Read for CustomStream
impl<R, W> Read for CustomStream<R, W>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.reader.read(buf)
    }
}

// Implement Write for CustomStream
impl<R, W> Write for CustomStream<R, W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}