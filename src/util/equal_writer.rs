use std::io::IoResult;

/// A `Writer` that writes an exact number of bytes to a sub-writer.
pub struct EqualWriter<W> {
    writer: W,
    size: uint,
}

impl<W: Writer> EqualWriter<W> {
    pub fn new(writer: W, size: uint) -> EqualWriter<W> {
        EqualWriter {
            writer: writer,
            size: size,
        }
    }
}

impl<W: Writer> Writer for EqualWriter<W> {
    // TODO: what if there is a write error?
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        if buf.len() > self.size {
            let sz = self.size;
            let res = self.write(buf.slice_to(sz));
            self.size = 0;
            res
        } else {
            self.size -= buf.len();
            self.writer.write(buf)
        }
    }
}

#[unsafe_destructor]
impl<W: Writer> Drop for EqualWriter<W> {
    fn drop(&mut self) {
        for _ in range(0, self.size) {
            self.writer.write_u8(0).ok();       // TODO: how to handle error?
        }
    }
}
