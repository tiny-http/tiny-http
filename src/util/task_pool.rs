use std::sync::Arc;
use std::sync::mpsc_queue::Queue;

pub struct TaskPool {
    free_tasks: Arc<Queue<Sender<proc():Send>>>,
}

impl TaskPool {
    pub fn new() -> TaskPool {
        let mut pool = TaskPool {
            free_tasks: Arc::new(Queue::new()),
        };

        // adding one thread per CPU
        {
            use std::os;
            use std::cmp;
            for _ in range(0, cmp::min(1u, os::num_cpus() - 1)) {
                pool.add_thread(None);
            }
        }

        pool
    }

    pub fn spawn(&mut self, mut code: proc():Send) {
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

    fn add_thread(&mut self, initial_fn: Option<proc():Send>) {
        use std::io::timer::Timer;

        let queue = self.free_tasks.clone();

        spawn(proc() {
            let mut timer = Timer::new().unwrap();

            loop {
                let (tx, rx) = channel();
                queue.push(tx);

                let timeout = timer.oneshot(5000);
                select! {
                    next_fn = rx.recv() => next_fn(),
                    _ = timeout.recv() => break
                }
            }
        })
    }
}
