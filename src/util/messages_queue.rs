use std::collections::VecDeque;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex, Condvar};

pub struct MessagesQueue<T> where T: Send {
    queue: Mutex<VecDeque<T>>,
    condvar: Condvar,
}

impl<T> MessagesQueue<T> where T: Send {
    pub fn with_capacity(capacity: usize) -> Arc<MessagesQueue<T>> {
        Arc::new(MessagesQueue {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar: Condvar::new(),
        })
    }

    /// Pushes an element to the queue.
    pub fn push(&self, value: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(value);
        self.condvar.notify_one();
    }

    /// Pops an element. Blocks until one is available.
    pub fn pop(&self) -> T {
        let mut queue = self.queue.lock().unwrap();

        loop {
            if let Some(elem) = queue.pop_front() {
                return elem;
            }

            queue = self.condvar.wait(queue).unwrap();
        }
    }

    /// Tries to pop an element without blocking.
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    /// Tries to pop an element without blocking
    /// more than the specified timeout duration
    pub fn pop_timeout(&self, timeout: Duration) -> Option<T> {
        let mut queue = self.queue.lock().unwrap();
        let mut duration = timeout;
        loop {
            if let Some(elem) = queue.pop_front() {
                return Some(elem);
            }
            let now = Instant::now();
            let (_queue, result) = self.condvar.wait_timeout(queue, timeout).unwrap();
            queue = _queue;
            let sleep_time = now.elapsed();
            duration = if duration > sleep_time { duration - sleep_time } else { Duration::from_millis(0) };
            if result.timed_out() ||
               (duration.as_secs() == 0 && duration.subsec_nanos() < 1000000) {
                return None;
            }
        }
    }
}
