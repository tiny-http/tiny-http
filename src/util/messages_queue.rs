use std::collections::VecDeque;
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
}
