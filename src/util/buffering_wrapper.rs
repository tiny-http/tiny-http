use std::io::{BufWriter, Result as IoResult, Write};

pub enum MaybeBufferedWriter<W: Write> {
    Buffered(BufWriter<W>),
    Unbuffered(W),
}

impl<W: Write> Write for MaybeBufferedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        match self {
            MaybeBufferedWriter::Buffered(w) => w.write(buf),
            MaybeBufferedWriter::Unbuffered(w) => w.write(buf),
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        match self {
            MaybeBufferedWriter::Buffered(w) => w.write_all(buf),
            MaybeBufferedWriter::Unbuffered(w) => w.write_all(buf),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self {
            MaybeBufferedWriter::Buffered(w) => w.flush(),
            MaybeBufferedWriter::Unbuffered(w) => w.flush(),
        }
    }
}
