use std::sync::Arc;
use std::sync::mpsc_queue::Queue;

/// Manages a collection of threads.
///
/// A new thread is created every time all the existing threads are full.
/// Any idle thread will automatically die after 5 seconds.
pub struct TaskPool {
    free_tasks: Arc<Queue<Sender<proc():Send>>>,
}

/// Number of milliseconds after which an idle thread dies.
static THREAD_IDLE_DIEAFTER: u64 = 5000;

impl TaskPool {
    pub fn new() -> TaskPool {
        TaskPool {
            free_tasks: Arc::new(Queue::new()),
        }
    }

    /// Executes a function in a thread.
    /// If no thread is available, spawns a new one.
    pub fn spawn(&self, mut code: proc():Send) {
        use std::task;
        use std::sync::mpsc_queue::{Data, Empty, Inconsistent};

        loop {
            match self.free_tasks.pop() {
                Data(sender) => {
                    match sender.send_opt(code) {
                        Ok(_) => return,
                        Err(org) => code = org
                    }
                },
                Inconsistent =>
                    task::deschedule(),
                Empty => {
                    self.add_thread(Some(code));
                    return
                }
            }
        }
    }

    fn add_thread(&self, initial_fn: Option<proc():Send>) {
        use std::io::timer::Timer;

        let queue = self.free_tasks.clone();

        spawn(proc() {
            let mut timer = Timer::new().unwrap();

            if initial_fn.is_some() {
                let f = initial_fn.unwrap();
                f();
            }

            loop {
                let (tx, rx) = channel();
                queue.push(tx);

                let timeout = timer.oneshot(THREAD_IDLE_DIEAFTER);
                select! {
                    next_fn = rx.recv() => next_fn(),
                    _ = timeout.recv() => break
                }
            }
        })
    }
}
