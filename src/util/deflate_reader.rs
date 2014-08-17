use std::io::IoResult;
use flate;

pub struct DeflateReader<R> {
    reader: R,
    buffer: Option<Vec<u8>>,
}

impl<R: Reader> DeflateReader<R> {
    pub fn new(reader: R) -> DeflateReader<R> {
        DeflateReader {
            reader: reader,
            buffer: None,
        }
    }
}

impl<R: Reader> Reader for DeflateReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        // filling the buffer if we don't have any
        if self.buffer.is_none() {
            let data = try!(self.reader.read_to_end());

            let result = match flate::deflate_bytes(data.as_slice()) {
                Some(d) => d,
                None => {
                    use std::io;
                    use std::io::InvalidInput;
                    return Err(io::standard_error(InvalidInput));
                }
            };

            self.buffer = Some(Vec::from_slice(result.as_slice()));
        }

        // if our buffer exists but is empty, we reached EOF
        if self.buffer.as_ref().unwrap().len() == 0 {
            use std::io;
            use std::io::EndOfFile;
            return Err(io::standard_error(EndOfFile));
        }

        // copying the buffer to the output
        let qty = {
            use std::slice::MutableCloneableSlice;
            buf.clone_from_slice(self.buffer.as_ref().unwrap().as_slice())
        };

        self.buffer = Some(Vec::from_slice(self.buffer.as_ref().unwrap().slice_from(qty)));
        Ok(qty)
    }
}
