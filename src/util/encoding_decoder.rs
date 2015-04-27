use std::io::Result as IoResult;
use std::io::{Cursor, Read};
use encoding::{DecoderTrap, Encoding};

// TODO: for the moment the first call to read() reads the whole
//  underlying reader at once and decodes it

pub struct EncodingDecoder<R> {
	reader: R,
	encoding: &'static Encoding,
	content: Option<Cursor>,
}

impl<R> EncodingDecoder<R> where R: Read {
	pub fn new(reader: R, encoding: &'static Encoding) -> EncodingDecoder<R> {
		EncodingDecoder {
			reader: reader,
			encoding: encoding,
			content: None,
		}
	}
}

impl<R> Read for EncodingDecoder<R> where R: Read {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		if self.content.is_none() {
			let mut data = Vec::with_capacity(0);
			try!(self.reader.read_to_end(&mut data));

			let result = match self.encoding.decode(&data, DecoderTrap::Strict) {
				Ok(s) => s,
				Err(_) => panic!(), // FIXME: return Err(old_io::standard_error(old_io::InvalidInput))
			};

			self.content = Some(Cursor::new(result.into_bytes()));
		}

		if let Some(ref mut content) = self.content {
			content.read(buf)
		} else {
			unreachable!();
		}
	}
}
