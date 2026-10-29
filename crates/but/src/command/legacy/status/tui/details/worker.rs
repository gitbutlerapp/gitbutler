use std::sync::{Arc, Condvar, Mutex, atomic::AtomicBool};

/// A background worker that performs arbitrary jobs while maintaining a "next job".
///
/// Submitting a new job to the worker replaces any previous jobs. It essentially has a queue with
/// a max length of 1.
///
/// This exists to prevent a scenario where we're performing some job (such as loading diffs) which
/// holds an exclusive lock on some resource that block other jobs and builds a large queue.
///
/// The TUI's detail view needs this because it starts loading diffs immediately when an item is
/// selected so if you scroll quickly we don't wanna build a big queue of diffs to load, we just
/// wanna finish the current one and load whatever the next diff is. LIFO style.
pub struct Worker {
    shared: Arc<Shared>,
    exclusives: Arc<Mutex<Exclusives>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Shared {
    condvar: Condvar,
    is_busy: AtomicBool,
    worker_dropped: AtomicBool,
}

#[derive(Default)]
struct Exclusives {
    next_job: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl Worker {
    pub fn new() -> Self {
        let worker = Self {
            shared: Default::default(),
            exclusives: Default::default(),
        };

        if cfg!(test) {
            // dont bother with threads during tests since that makes timing non-deterministic
        } else {
            let shared = Arc::clone(&worker.shared);
            let exclusives = Arc::clone(&worker.exclusives);
            std::thread::spawn(move || {
                run_worker(shared, exclusives);
            });
        }

        worker
    }

    pub fn replace_next_job<F>(&mut self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.replace_next_job_inner(Some(Box::new(job)));
    }

    pub fn clear_next_job(&mut self) {
        self.replace_next_job_inner(None);
    }

    fn replace_next_job_inner(&mut self, job: Option<Box<dyn FnOnce() + Send + 'static>>) {
        if cfg!(test) {
            // dont bother with threads during tests since that makes timing non-deterministic
            if let Some(job) = job {
                job();
            }
        } else {
            let has_job = job.is_some();

            let mut lock = self.exclusives.lock().unwrap();
            lock.next_job = job;
            drop(lock);

            if has_job {
                self.shared.condvar.notify_one();
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.shared
            .is_busy
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shared
            .worker_dropped
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.shared.condvar.notify_one();
    }
}

fn run_worker(shared: Arc<Shared>, exclusives: Arc<Mutex<Exclusives>>) {
    loop {
        if shared
            .worker_dropped
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            break;
        }

        let mut lock = exclusives.lock().unwrap();
        let next_job = if let Some(job) = lock.next_job.take() {
            drop(lock);
            Some(job)
        } else {
            let mut lock = shared.condvar.wait(lock).unwrap();
            let job = lock.next_job.take();
            drop(lock);
            job
        };

        if shared
            .worker_dropped
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            break;
        }

        if let Some(job) = next_job {
            shared
                .is_busy
                .store(true, std::sync::atomic::Ordering::SeqCst);
            job();
            shared
                .is_busy
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
