use std::io::{IoResult, MemReader};
use encoding::{DecoderTrap, Encoding};

// TODO: for the moment the first call to read() reads the whole
//  underlying reader at once and decodes it
#[experimental]
pub struct EncodingDecoder<R> {
	reader: R,
	encoding: &'static Encoding + 'static,
	content: Option<MemReader>,
}

impl<R: Reader> EncodingDecoder<R> {
	#[experimental]
	pub fn new(reader: R, encoding: &'static Encoding + 'static) -> EncodingDecoder<R> {
		EncodingDecoder {
			reader: reader,
			encoding: encoding,
			content: None,
		}
	}
}

impl<R: Reader> Reader for EncodingDecoder<R> {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
		if self.content.is_none() {
			use std::io;

			let data = try!(self.reader.read_to_end());

			let result = match self.encoding.decode(data.as_slice(), DecoderTrap::Strict) {
				Ok(s) => s,
				Err(_) => return Err(io::standard_error(io::InvalidInput))
			};

			self.content = Some(MemReader::new(result.into_bytes()));
		}

		assert!(self.content.is_some());
		self.content.as_mut().unwrap().read(buf)
	}
}
