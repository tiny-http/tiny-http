use std::io::Read;
use std::io::Result as IoResult;

pub struct DeflateReader<R> {
    reader: R,
    buffer: Option<Vec<u8>>,
}

impl<R> DeflateReader<R> where R: Read {
    pub fn new(reader: R) -> DeflateReader<R> {
        DeflateReader {
            reader: reader,
            buffer: None,
        }
    }
}

impl<R> Read for DeflateReader<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // filling the buffer if we don't have any
        if self.buffer.is_none() {
            let mut data = Vec::with_capacity(0);
            try!(self.reader.read_to_end(&mut data));

            // FIXME: 
            let result = data;
            //let result = flate::deflate_bytes(data);

            self.buffer = Some(result);
        }

        // if our buffer exists but is empty, we reached EOF
        if self.buffer.as_ref().unwrap().len() == 0 {
            return Ok(0);
        }

        // copying the buffer to the output
        let qty = {
            buf.clone_from_slice(self.buffer.as_ref().unwrap())
        };

        self.buffer = Some((&self.buffer.as_ref().unwrap()[qty..]).to_vec());
        Ok(qty)
    }
}
