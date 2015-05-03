// Copyright 2015 The tiny-http Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Result as IoResult;
use std::io::{Cursor, Read};
use encoding::{DecoderTrap, Encoding};

// TODO: for the moment the first call to read() reads the whole
//  underlying reader at once and decodes it

pub struct EncodingDecoder<R> {
    reader: R,
    encoding: &'static Encoding,
    content: Option<Cursor<Vec<u8>>>,
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
