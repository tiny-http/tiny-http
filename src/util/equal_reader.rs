use std::io::IoResult;
use std::io::util::LimitReader;

/// A `Reader` that reads exactly the number of bytes from a sub-reader.
/// 
/// If the limit is reached, it returns EOF. If the limit is not reached
///  when the destructor is called, the remaining bytes will be read and
///  thrown away.
pub struct EqualReader<R> {
    reader: Option<LimitReader<R>>,
    last_read_signal: Sender<IoResult<()>>,
}

impl<R: Reader> EqualReader<R> {
    pub fn new(reader: R, size: uint) -> (EqualReader<R>, Receiver<IoResult<()>>) {
        let (tx, rx) = channel();

        let r = EqualReader {
            reader: Some(LimitReader::new(reader, size)),
            last_read_signal: tx,
        };

        (r, rx)
    }
}

impl<R: Reader> Reader for EqualReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.reader.as_mut().unwrap().read(buf)
    }
}

#[unsafe_destructor]
impl<R: Reader> Drop for EqualReader<R> {
    fn drop(&mut self) {
        let remaining_to_read = self.reader.as_ref().unwrap().limit();

        let mut stored = None;
        ::std::mem::swap(&mut stored, &mut self.reader);

        let mut subreader = stored.unwrap().unwrap();
        let res = subreader.read_exact(remaining_to_read).map(|_| ());

        self.last_read_signal.send_opt(res).ok();
    }
}


#[cfg(test)]
mod test {
    use super::EqualReader;

    #[test]
    fn test_limit() {
        use std::io::MemReader;

        let mut org_reader = MemReader::new("hello world".to_string().into_bytes());

        {
            let (mut equal_reader, _) = EqualReader::new(org_reader.by_ref(), 5);

            assert_eq!(equal_reader.read_to_string().unwrap().as_slice(), "hello");
        }

        assert_eq!(org_reader.read_to_string().unwrap().as_slice(), " world");
    }

    #[test]
    fn test_not_enough() {
        use std::io::MemReader;

        let mut org_reader = MemReader::new("hello world".to_string().into_bytes());

        {
            let (mut equal_reader, _) = EqualReader::new(org_reader.by_ref(), 5);

            assert_eq!(equal_reader.read_u8().unwrap(), b'h');
        }

        assert_eq!(org_reader.read_to_string().unwrap().as_slice(), " world");
    }
}
