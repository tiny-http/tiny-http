use std::old_io::{IoResult, MemReader};
use std::old_io::Reader;
use encoding::{DecoderTrap, Encoding};

// TODO: for the moment the first call to read() reads the whole
//  underlying reader at once and decodes it
#[unstable]
pub struct EncodingDecoder<R> {
	reader: R,
	encoding: &'static Encoding,
	content: Option<MemReader>,
}

impl<R: Reader> EncodingDecoder<R> {
	#[unstable]
	pub fn new(reader: R, encoding: &'static Encoding) -> EncodingDecoder<R> {
		EncodingDecoder {
			reader: reader,
			encoding: encoding,
			content: None,
		}
	}
}

impl<R: Reader> Reader for EncodingDecoder<R> {
	fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
		if self.content.is_none() {
			use std::old_io;

			let data = try!(self.reader.read_to_end());

			let result = match self.encoding.decode(data.as_slice(), DecoderTrap::Strict) {
				Ok(s) => s,
				Err(_) => return Err(old_io::standard_error(old_io::InvalidInput))
			};

			self.content = Some(MemReader::new(result.into_bytes()));
		}

		assert!(self.content.is_some());
		self.content.as_mut().unwrap().read(buf)
	}
}
