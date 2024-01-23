use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

/// Manages a collection of threads.
///
/// A new thread is created every time all the existing threads are full.
/// Any idle thread will automatically die after a few seconds.
pub struct TaskPool {
    sharing: Arc<Sharing>,
    thread_count: usize, // add a field to store the fixed thread count
}

struct Sharing {
    // list of the tasks to be done by worker threads
    todo: Mutex<VecDeque<Box<dyn FnMut() + Send>>>,

    // condvar that will be notified whenever a task is added to `todo`
    condvar: Condvar,

    // number of total worker threads running
    active_tasks: AtomicUsize,

    // number of idle worker threads
    waiting_tasks: AtomicUsize,
}

/// Minimum number of active threads.
static MIN_THREADS: usize = 4;

struct Registration<'a> {
    nb: &'a AtomicUsize,
}

impl<'a> Registration<'a> {
    fn new(nb: &'a AtomicUsize) -> Registration<'a> {
        nb.fetch_add(1, Ordering::Release);
        Registration { nb }
    }
}

impl<'a> Drop for Registration<'a> {
    fn drop(&mut self) {
        self.nb.fetch_sub(1, Ordering::Release);
    }
}

impl TaskPool {
    pub fn new(thread_count: usize) -> TaskPool {
        let pool = TaskPool {
            sharing: Arc::new(Sharing {
                todo: Mutex::new(VecDeque::new()),
                condvar: Condvar::new(),
                active_tasks: AtomicUsize::new(0),
                waiting_tasks: AtomicUsize::new(0),
            }),
            thread_count, // store the fixed thread count
        };

        for _ in 0..thread_count {
            pool.add_thread(None);
        }

        pool
    }

    /// Executes a function in a thread.
    /// If no thread is available, spawns a new one.
    pub fn spawn(&self, code: Box<dyn FnMut() + Send>) {
        let mut queue = self.sharing.todo.lock().unwrap();

        if self.sharing.active_tasks.load(Ordering::Acquire) < self.thread_count {
            if self.sharing.waiting_tasks.load(Ordering::Acquire) == 0 {
                self.add_thread(Some(code));
            } else {
                queue.push_back(code);
                self.sharing.condvar.notify_one();
            }
        } else {
            // if the number of active threads is equal to the fixed thread count,
            queue.push_back(code);
            self.sharing.condvar.notify_one();
        }
    }

    fn add_thread(&self, initial_fn: Option<Box<dyn FnMut() + Send>>) {
        //  if the number of active threads is greater than the fixed thread count, return
        if self.sharing.active_tasks.load(Ordering::Acquire) >= self.thread_count {
            return;
        }

        let sharing = self.sharing.clone();

        thread::spawn(move || {
            let _active_guard = Registration::new(&sharing.active_tasks);

            // execute the initial function if there is one
            if let Some(mut f) = initial_fn {
                f();
            }

            loop {
                let maybe_task = {
                    let mut todo = sharing.todo.lock().unwrap();

                    loop {
                        match todo.pop_front() {
                            Some(task) => break Some(task),
                            None => {
                                let _waiting_guard = Registration::new(&sharing.waiting_tasks);
                                // if the number of active threads is greater than the fixed thread count, return
                                if sharing.active_tasks.load(Ordering::Acquire) <= 1 {
                                    todo = sharing.condvar.wait(todo).unwrap();
                                } else {
                                    // wait for some seconds
                                    let (new_todo, timeout) = sharing
                                        .condvar
                                        .wait_timeout(todo, Duration::from_secs(5))
                                        .unwrap();
                                    todo = new_todo;
                                    if timeout.timed_out() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                };

                // execute the task
                if let Some(mut task) = maybe_task {
                    task();
                }
            }
        });
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        self.sharing
            .active_tasks
            .store(999_999_999, Ordering::Release);
        self.sharing.condvar.notify_all();
    }
}
