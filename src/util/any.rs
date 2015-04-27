use std::io::Result as IoResult;
use std::io::{Read, Write};

pub struct AnyReader {
    reader: Box<Read + Send + 'static>,
}

pub struct AnyWriter {
    writer: Box<Write + Send + 'static>,
}

impl AnyReader {
    pub fn new(reader: Box<Read + Send + 'static>) -> AnyReader {
        AnyReader {
            reader: reader,
        }
    }

    pub fn unwrap(self) -> Box<Read + Send + 'static> {
        self.reader
    }
}

impl AnyWriter {
    pub fn new(writer: Box<Write + Send + 'static>) -> AnyWriter {
        AnyWriter {
            writer: writer,
        }
    }

    pub fn unwrap(self) -> Box<Write + Send + 'static> {
        self.writer
    }
}

impl Read for AnyReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.reader.read(buf)
    }
}

impl Write for AnyWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
