use std::io::IoResult;
use std::sync;
use std::sync::{Arc, Mutex};

pub struct SequentialReaderBuilder<R> {
    reader: Arc<Mutex<R>>,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialReader<R> {
    trigger: Option<sync::Future<()>>,
    reader: Arc<Mutex<R>>,
    on_finish: Sender<()>,
}

pub struct SequentialWriterBuilder<W> {
    writer: Arc<Mutex<W>>,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialWriter<W> {
    trigger: Option<sync::Future<()>>,
    writer: Arc<Mutex<W>>,
    on_finish: Sender<()>,
}

impl<R: Reader + Send> SequentialReaderBuilder<R> {
    pub fn new(reader: R) -> SequentialReaderBuilder<R> {
        SequentialReaderBuilder {
            reader: Arc::new(Mutex::new(reader)),
            next_trigger: None,
        }
    }
}

impl<W: Writer + Send> SequentialWriterBuilder<W> {
    pub fn new(writer: W) -> SequentialWriterBuilder<W> {
        SequentialWriterBuilder {
            writer: Arc::new(Mutex::new(writer)),
            next_trigger: None,
        }
    }
}

impl<R: Reader + Send> Iterator<SequentialReader<R>> for SequentialReaderBuilder<R> {
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

impl<W: Writer + Send> Iterator<SequentialWriter<W>> for SequentialWriterBuilder<W> {
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

impl<R: Reader + Send> Reader for SequentialReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<uint> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.reader.lock().read(buf)
    }
}

impl<W: Writer + Send> Writer for SequentialWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<()> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.writer.lock().write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.writer.lock().flush()
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
