use std::io::IoResult;

pub struct CustomStream<R, W> {
    reader: R,
    writer: W,
}

impl<R: Reader, W: Writer> CustomStream<R, W> {
    pub fn new(reader: R, writer: W) -> CustomStream<R, W> {
        CustomStream {
            reader: reader,
            writer: writer,
        }
    }
}

impl<R: Reader, W> Reader for CustomStream<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.reader.read(buf)
    }
}

impl<R, W: Writer> Writer for CustomStream<R, W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
