use std::sync::Arc;
use std::sync::atomic::AtomicUint;
use std::sync::mpsc_queue::Queue;
use std::time::duration::Duration;

/// Manages a collection of threads.
///
/// A new thread is created every time all the existing threads are full.
/// Any idle thread will automatically die after 5 seconds.
pub struct TaskPool {
    free_tasks: Arc<Queue<Sender<proc():Send>>>,
    active_tasks: Arc<AtomicUint>,
}

/// Minimum number of active threads.
static MIN_THREADS: uint = 4;


struct Registration {
    nb: Arc<AtomicUint>
}

impl Registration {
    fn new(nb: Arc<AtomicUint>) -> Registration {
        use std::sync::atomic::Relaxed;
        nb.fetch_add(1, Relaxed);
        Registration { nb: nb }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        use std::sync::atomic::Relaxed;
        self.nb.fetch_sub(1, Relaxed);
    }
}

/// Returns the duration after which an idle thread dies.
#[inline(always)]
fn get_idle_thread_dieafter() -> Duration {
    Duration::seconds(5)
}

impl TaskPool {
    pub fn new() -> TaskPool {
        let pool = TaskPool {
            free_tasks: Arc::new(Queue::new()),
            active_tasks: Arc::new(AtomicUint::new(0)),
        };

        for _ in range(0, MIN_THREADS) {
            pool.add_thread(None)
        }

        pool
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
        let active_tasks = self.active_tasks.clone();

        spawn(proc() {
            let active_tasks = active_tasks;
            let _ = Registration::new(active_tasks.clone());
            let mut timer = Timer::new().unwrap();

            if initial_fn.is_some() {
                let f = initial_fn.unwrap();
                f();
            }

            loop {
                let (tx, rx) = channel();
                queue.push(tx);

                let timeout = timer.oneshot(get_idle_thread_dieafter());
                select! {
                    next_fn = rx.recv() => next_fn(),
                    _ = timeout.recv() => {
                        use std::sync::atomic::Relaxed;
                        if active_tasks.load(Relaxed) >= MIN_THREADS {
                            break
                        }
                    }
                }
            }
        })
    }
}

impl Drop for TaskPool {
    fn drop(&mut self) {
        use std::sync::atomic::Relaxed;
        self.active_tasks.store(999999999, Relaxed);
    }
}
