use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::sync::atomic::{Ordering, AtomicUsize};
use std::sync::mpsc::channel;
use std::collections::VecDeque;
use std::thread;

/// Manages a collection of threads.
///
/// A new thread is created every time all the existing threads are full.
/// Any idle thread will automatically die after a few seconds.
pub struct TaskPool {
    free_tasks: Arc<Mutex<VecDeque<Sender<Box<FnMut() + Send>>>>>,
    active_tasks: Arc<AtomicUsize>,
}

/// Minimum number of active threads.
static MIN_THREADS: usize = 4;

struct Registration {
    nb: Arc<AtomicUsize>
}

impl Registration {
    fn new(nb: Arc<AtomicUsize>) -> Registration {
        nb.fetch_add(1, Ordering::Relaxed);
        Registration { nb: nb }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.nb.fetch_sub(1, Ordering::Relaxed);
    }
}

impl TaskPool {
    pub fn new() -> TaskPool {
        let pool = TaskPool {
            free_tasks: Arc::new(Mutex::new(VecDeque::new())),
            active_tasks: Arc::new(AtomicUsize::new(0)),
        };

        for _ in 0..MIN_THREADS {
            pool.add_thread(None)
        }

        pool
    }

    /// Executes a function in a thread.
    /// If no thread is available, spawns a new one.
    pub fn spawn(&self, mut code: Box<FnMut() + Send>) {
        let mut queue = self.free_tasks.lock().unwrap();

        loop {
            if let Some(sender) = queue.pop_front() {
                match sender.send(code) {
                    Ok(_) => return,
                    Err(err) => { code = err.0; continue; }
                };
            } else {
                self.add_thread(Some(code));
                return;
            }
        }
    }

    fn add_thread(&self, initial_fn: Option<Box<FnMut() + Send>>) {
        let queue = self.free_tasks.clone();
        let active_tasks = self.active_tasks.clone();

        thread::spawn(move || {
            let active_tasks = active_tasks;
            let _ = Registration::new(active_tasks.clone());

            if initial_fn.is_some() {
                let mut f = initial_fn.unwrap();
                f();
            }

            let mut timeout_cycles = 5000 / 3;

            loop {
                let (tx, rx) = channel();

                {
                    let mut queue = queue.lock().unwrap();
                    queue.push_back(tx);
                }

                thread::sleep_ms(3);

                match rx.try_recv() {
                    Ok(mut f) => {
                        f();
                        timeout_cycles = 5000 / 3;
                    },
                    _ => {
                        timeout_cycles -= 1;

                        if timeout_cycles == 0 && active_tasks.load(Ordering::Relaxed)
                                                  >= MIN_THREADS
                        {
                            break;
                        }
                    }
                };
            }
        });
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        self.active_tasks.store(999999999, Ordering::Relaxed);
    }
}
