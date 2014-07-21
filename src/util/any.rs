use std::io::IoResult;

pub struct AnyReader {
    reader: Box<Reader + Send>
}

pub struct AnyWriter {
    writer: Box<Writer + Send>
}

impl AnyReader {
    pub fn new(reader: Box<Reader + Send>) -> AnyReader {
        AnyReader {
            reader: reader,
        }
    }

    pub fn unwrap(self) -> Box<Reader + Send> {
        self.reader
    }
}

impl AnyWriter {
    pub fn new(writer: Box<Writer + Send>) -> AnyWriter {
        AnyWriter {
            writer: writer,
        }
    }

    pub fn unwrap(self) -> Box<Writer + Send> {
        self.writer
    }
}

impl Reader for AnyReader {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.reader.read(buf)
    }
}

impl Writer for AnyWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
