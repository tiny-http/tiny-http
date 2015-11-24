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

pub use self::custom_stream::CustomStream;
pub use self::encoding_decoder::EncodingDecoder;
pub use self::equal_reader::EqualReader;
pub use self::messages_queue::MessagesQueue;
pub use self::refined_tcp_stream::RefinedTcpStream;
pub use self::sequential::{SequentialReaderBuilder, SequentialReader};
pub use self::sequential::{SequentialWriterBuilder, SequentialWriter};
pub use self::task_pool::TaskPool;

use std::str::FromStr;

mod custom_stream;
mod encoding_decoder;
mod equal_reader;
mod messages_queue;
mod refined_tcp_stream;
mod sequential;
mod task_pool;

/// Parses a the value of a header.
/// Suitable for `Accept-*`, `TE`, etc.
/// 
/// For example with `text/plain, image/png; q=1.5` this function would 
/// return `[ ("text/plain", 1.0), ("image/png", 1.5) ]`
pub fn parse_header_value<'a>(input: &'a str) -> Vec<(&'a str, f32)> {
    input.split(',').filter_map(|elem| {
        let mut params = elem.split(';');

        let t = params.next();
        if t.is_none() { return None; }

        let mut value = 1.0f32;

        for p in params {
            if p.trim_left().starts_with("q=") {
                match FromStr::from_str(&p.trim_left()[2..].trim()) {
                    Ok(val) => { value = val; break },
                    _ => ()
                }
            }
        }

        Some((t.unwrap().trim(), value))

    }).collect()
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_header() {
        let result = super::parse_header_value("text/html, text/plain; q=1.5 , image/png ; q=2.0");

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "text/html");
        assert_eq!(result[0].1, 1.0);
        assert_eq!(result[1].0, "text/plain");
        assert_eq!(result[1].1, 1.5);
        assert_eq!(result[2].0, "image/png");
        assert_eq!(result[2].1, 2.0);
    }
}
