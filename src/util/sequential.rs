use std::io::Result as IoResult;
use std::io::{Read, Write};

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;
use std::sync::{self, Arc, Mutex};

pub struct SequentialReaderBuilder<R> where R: Read + Send {
    reader: Arc<Mutex<R>>,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialReader<R> where R: Read + Send {
    trigger: Option<sync::Future<()>>,
    reader: Arc<Mutex<R>>,
    on_finish: Sender<()>,
}

pub struct SequentialWriterBuilder<W> where W: Write + Send {
    writer: Arc<Mutex<W>>,
    next_trigger: Option<sync::Future<()>>,
}

pub struct SequentialWriter<W> where W: Write + Send {
    trigger: Option<sync::Future<()>>,
    writer: Arc<Mutex<W>>,
    on_finish: Sender<()>,
}

impl<R: Read + Send> SequentialReaderBuilder<R> {
    pub fn new(reader: R) -> SequentialReaderBuilder<R> {
        SequentialReaderBuilder {
            reader: Arc::new(Mutex::new(reader)),
            next_trigger: None,
        }
    }
}

impl<W: Write + Send> SequentialWriterBuilder<W> {
    pub fn new(writer: W) -> SequentialWriterBuilder<W> {
        SequentialWriterBuilder {
            writer: Arc::new(Mutex::new(writer)),
            next_trigger: None,
        }
    }
}

impl<R: Read + Send> Iterator for SequentialReaderBuilder<R> {
    type Item = SequentialReader<R>;
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

impl<W: Write + Send> Iterator for SequentialWriterBuilder<W> {
    type Item = SequentialWriter<W>;
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

impl<R: Read + Send> Read for SequentialReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.reader.lock().unwrap().read(buf)
    }
}

impl<W: Write + Send> Write for SequentialWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.writer.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.trigger.as_mut().map(|v| v.get());
        self.trigger = None;

        self.writer.lock().unwrap().flush()
    }
}

impl<R> Drop for SequentialReader<R> where R: Read + Send {
    fn drop(&mut self) {
        self.on_finish.send(()).ok();
    }
}

impl<W> Drop for SequentialWriter<W> where W: Write + Send {
    fn drop(&mut self) {
        self.on_finish.send(()).ok();
    }
}
