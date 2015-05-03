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
use std::io::{Read, Write};

pub struct CustomStream<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> CustomStream<R, W> where R: Read, W: Write {
    pub fn new(reader: R, writer: W) -> CustomStream<R, W> {
        CustomStream {
            reader: reader,
            writer: writer,
        }
    }
}

impl<R, W> Read for CustomStream<R, W> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.reader.read(buf)
    }
}

impl<R, W> Write for CustomStream<R, W> where W: Write {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.writer.flush()
    }
}
