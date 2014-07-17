pub use self::chunks_encoder::ChunksEncoder;
pub use self::closable_tcp_acceptor::ClosableTcpAcceptor;
pub use self::closable_tcp_stream::ClosableTcpStream;
pub use self::encoding_decoder::EncodingDecoder;
pub use self::equal_reader::EqualReader;
pub use self::task_pool::TaskPool;

mod chunks_encoder;
mod closable_tcp_acceptor;
mod closable_tcp_stream;
mod encoding_decoder;
mod equal_reader;
mod task_pool;

/// Parses a the value of a header.
/// Suitable for Accept-*, TE, etc.
/// 
/// For example with "text/plain, image/png; q=1.5" this function would 
///  return [ ("text/plain", 1.0), ("image/png", 1.5) ]
pub fn parse_header_value<'a>(input: &'a str) -> Vec<(&'a str, f32)> {
    let mut result = Vec::new();

    for elem in input.split(',') {
        let mut params = elem.split(';');

        let t = params.next();
        if t.is_none() { continue }

        let mut value = 1.0f32;

        for p in params {
            if p.trim_left().starts_with("q=") {
                match from_str(p.trim_left().slice_from(2).trim()) {
                    Some(val) => { value = val; break },
                    _ => ()
                }
            }
        }

        result.push((t.unwrap().trim(), value));
    }

    result
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_header() {
        let result = super::parse_header_value("text/html, text/plain; q=1.5 , image/png ; q=2.0");

        assert_eq!(result.len(), 3);
        assert_eq!(result.get(0).ref0(), &"text/html");
        assert_eq!(result.get(0).ref1(), &1.0);
        assert_eq!(result.get(1).ref0(), &"text/plain");
        assert_eq!(result.get(1).ref1(), &1.5);
        assert_eq!(result.get(2).ref0(), &"image/png");
        assert_eq!(result.get(2).ref1(), &2.0);
    }
}
