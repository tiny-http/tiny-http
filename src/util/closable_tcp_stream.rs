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

use std::io::{Read, Write};
use std::io::Result as IoResult;
use std::net::{SocketAddr, TcpStream, Shutdown};

pub struct ClosableTcpStream {
    stream: TcpStream,
    close_read: bool,
    close_write: bool,
}

impl ClosableTcpStream {
    pub fn new(stream: TcpStream, close_read: bool, close_write: bool) -> ClosableTcpStream {
        ClosableTcpStream {
            stream: stream,
            close_read: close_read,
            close_write: close_write,
        }
    }

    pub fn peer_addr(&mut self) -> IoResult<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl Drop for ClosableTcpStream {
    fn drop(&mut self) {
        if self.close_read {
            self.stream.shutdown(Shutdown::Read).ok();      // ignoring outcome
        }
        if self.close_write {
            self.stream.shutdown(Shutdown::Write).ok();     // ignoring outcome
        }
    }
}

impl Read for ClosableTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.stream.read(buf)
    }
}

impl Write for ClosableTcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.stream.flush()
    }
}
