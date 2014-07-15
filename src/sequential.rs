use std::io::IoResult;
use std::sync;

pub struct SequentialReaderBuilder<R> {
    reader: R,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialReader<R> {
    trigger: Option<sync::Future<()>>,
    reader: R,
    on_finish: Sender<()>,
}

pub struct SequentialWriterBuilder<W> {
    writer: W,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialWriter<W> {
    trigger: Option<sync::Future<()>>,
    writer: W,
    on_finish: Sender<()>,
}

impl<R: Reader> SequentialReaderBuilder<R> {
    pub fn new(reader: R) -> SequentialReaderBuilder<R> {
        SequentialReaderBuilder {
            reader: reader,
            next_trigger: None,
        }
    }
}

impl<W: Writer> SequentialWriterBuilder<W> {
    pub fn new(writer: W) -> SequentialWriterBuilder<W> {
        SequentialWriterBuilder {
            writer: writer,
            next_trigger: None,
        }
    }
}

impl<R: Reader + Clone> Iterator<SequentialReader<R>> for SequentialReaderBuilder<R> {
    fn next(&mut self) -> Option<SequentialReader<R>> {
        let (tx, rx) = channel();
        let mut next_next_trigger = Some(sync::Future::from_receiver(rx));
        ::std::mem::swap(&mut next_next_trigger, &mut self.next_trigger);

        Some(SequentialReader {
            trigger: next_next_trigger,
            reader: self.reader.clone(),
            on_finish: tx,
        })
    }
}

impl<W: Writer + Clone> Iterator<SequentialWriter<W>> for SequentialWriterBuilder<W> {
    fn next(&mut self) -> Option<SequentialWriter<W>> {
        let (tx, rx) = channel();
        let mut next_next_trigger = Some(sync::Future::from_receiver(rx));
        ::std::mem::swap(&mut next_next_trigger, &mut self.next_trigger);

        Some(SequentialWriter {
            trigger: next_next_trigger,
            writer: self.writer.clone(),
            on_finish: tx,
        })
    }
}

impl<R: Reader> Reader for SequentialReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.reader.read(buf)
    }
}

impl<W: Writer> Writer for SequentialWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.writer.write(buf)
    }
}

#[unsafe_destructor]
impl<R> Drop for SequentialReader<R> {
    fn drop(&mut self) {
        self.on_finish.send_opt(()).ok();
    }
}

#[unsafe_destructor]
impl<W> Drop for SequentialWriter<W> {
    fn drop(&mut self) {
        self.on_finish.send_opt(()).ok();
    }
}
