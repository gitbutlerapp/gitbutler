//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
#![cfg_attr(not(feature = "napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "napi", deny(unsafe_code))]
#![deny(missing_docs)]

#[cfg(feature = "legacy")]
pub mod legacy;

/// Functions for GitHub authentication.
pub mod github;

/// Functions for GitLab authentication.
pub mod gitlab;

/// Functions that take a branch as input.
pub mod branch;

/// Functions that operate commits
pub mod commit;

/// Functions that show what changed in various Git entities, like trees, commits and the worktree.
pub mod diff;

/// Types meant to be serialised to JSON, without degenerating information despite the need to be UTF-8 encodable.
/// EXPERIMENTAL
pub mod json;

/// Functions releated to platform detection and information.
pub mod platform;

/// Functions related to the generation of TS types out of schemas
pub mod schema;

pub mod panic_capture;

/// A module for proof-of-concepts
pub mod poc {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    #[cfg(feature = "napi")]
    use std::{
        collections::HashMap,
        sync::{Mutex, OnceLock, atomic::AtomicU32},
    };

    #[cfg(feature = "napi")]
    use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};

    #[cfg(feature = "napi")]
    static NEXT_TASK_ID: AtomicU32 = AtomicU32::new(1);
    #[cfg(feature = "napi")]
    static TASK_INTERRUPTS: OnceLock<Mutex<HashMap<u32, Arc<AtomicBool>>>> = OnceLock::new();

    #[cfg(feature = "napi")]
    fn task_interrupts() -> &'static Mutex<HashMap<u32, Arc<AtomicBool>>> {
        TASK_INTERRUPTS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[cfg(feature = "napi")]
    fn emit_event(callback: &ThreadsafeFunction<LongRunningEvent>, event: LongRunningEvent) {
        let _ = callback.call(Ok(event), ThreadsafeFunctionCallMode::NonBlocking);
    }

    /// Either the actual data that is more and more complete, or increments that can be merged
    /// into the actual data by the receiver.
    /// Sending all data whenever it changes is probably better.
    pub struct Data(pub usize);

    /// Set `duration` to decide how long the function should run, without blocking the caller.
    /// IRL the duration would be determined by the amount of work to be done.
    ///
    /// Use `progress` to see fine-grained progress information, either flat or as [`gix::NestedProgress`]
    /// so that a progress tree can be built for complex progress ['visualisations'](https://asciinema.org/a/315956).
    /// Please note that the underlying implementation, [`Prodash`](https://github.com/GitoxideLabs/prodash?tab=readme-ov-file),
    /// also provides renderers. However, there are other crates as well and there is no reason these shouldn't be used if
    /// these seem fitter.
    ///
    /// Use `should_interrupt` which should become `true` if the function should stop processing.
    /// It must live outside of `scope`, like any other value borrowed by a scoped thread.
    ///
    /// Return a receiver for the data (incremental or increasingly complete, depending on what's more suitable).
    ///
    /// # Cancellation
    ///
    /// The task can be stopped in two ways:
    ///
    /// - by dropping the receiver
    ///    - this is easy, but has the disadvantage that the sender only stops once it tries to send the next result and fails
    ///      doing so.
    /// - by setting `should_interrupt` to `true`
    ///    - this mechanism is fine-grained, and the callee is expected to pull the value often, so it will respond
    ///      swiftly to cancellation requests.
    ///
    /// # `[but_api]` Integration
    ///
    /// This function can't be `[but_api]` annotated until it learns how to deal with `duration` and more importantly,
    /// `progress`, the return channel and how to wire up `should_interrupt`. If we want, any of these could serve as markers
    /// to indicate long-runnning functions
    ///
    /// # Why not `async`?
    ///
    /// Our computations are not IO bound but compute bound, so there is no benefit to `async` or `tokio`.
    /// And I really, really want to avoid all the issues we will be getting when `async` is used, `but-ctx` and `gix::Repository`
    /// do not like `await` and require workarounds.
    ///
    /// At first, the integration should be implemented by hand (i.e. for NAPI) before it's generalised.
    pub fn long_running_non_blocking_scoped_thread<'scope, 'env>(
        scope: &'scope thread::Scope<'scope, 'env>,
        duration: Duration,
        progres: impl gix::Progress + 'env,
        should_interrupt: &'env AtomicBool,
    ) -> std::sync::mpsc::Receiver<anyhow::Result<Data>> {
        let (tx, rx) = mpsc::channel();
        scope.spawn(move || run_long_running_worker(duration, progres, should_interrupt, tx));
        rx
    }

    /// Kinds of events emitted to JavaScript while a long-running task executes.
    #[cfg(feature = "napi")]
    #[napi_derive::napi(string_enum)]
    pub enum LongRunningEventKind {
        /// The task produced an intermediate progress update.
        Progress,
        /// The task finished successfully.
        Done,
        /// The task stopped because interruption was requested.
        Cancelled,
        /// The task failed with an error.
        Error,
    }

    /// Event payload for Node callbacks.
    #[cfg(feature = "napi")]
    #[napi_derive::napi(object)]
    pub struct LongRunningEvent {
        /// The id of the task that emitted this event.
        pub task_id: u32,
        /// The event category.
        pub kind: LongRunningEventKind,
        /// The latest completed step if available.
        pub step: Option<u32>,
        /// An optional error message for failed tasks.
        pub message: Option<String>,
    }

    /// Start a long-running task and stream progress to `callback` via a ThreadsafeFunction.
    ///
    /// Returns the task id, which can be used to interrupt processing with
    /// [`long_running_cancel_tsfn()`].
    #[cfg(feature = "napi")]
    #[napi_derive::napi(js_name = "longRunningStartTsfn")]
    pub fn long_running_start_tsfn(
        duration_ms: u32,
        callback: ThreadsafeFunction<LongRunningEvent>,
    ) -> napi::Result<u32> {
        let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let should_interrupt = Arc::new(AtomicBool::new(false));

        let mut tasks = task_interrupts().lock().map_err(|_| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "task registry is poisoned".to_string(),
            )
        })?;
        tasks.insert(task_id, should_interrupt.clone());
        drop(tasks);

        let rx = long_running_non_blocking_thread(
            Duration::from_millis(u64::from(duration_ms)),
            gix::progress::Discard,
            should_interrupt.clone(),
        );

        thread::spawn(move || {
            let mut last_step = None;
            let mut failed = false;

            for result in rx {
                match result {
                    Ok(data) => {
                        let step = u32::try_from(data.0).unwrap_or(u32::MAX);
                        last_step = Some(step);
                        emit_event(
                            &callback,
                            LongRunningEvent {
                                task_id,
                                kind: LongRunningEventKind::Progress,
                                step: Some(step),
                                message: None,
                            },
                        );
                    }
                    Err(err) => {
                        failed = true;
                        emit_event(
                            &callback,
                            LongRunningEvent {
                                task_id,
                                kind: LongRunningEventKind::Error,
                                step: last_step,
                                message: Some(format!("{err:#}")),
                            },
                        );
                        break;
                    }
                }
            }

            if !failed {
                let kind = if should_interrupt.load(Ordering::Relaxed) {
                    LongRunningEventKind::Cancelled
                } else {
                    LongRunningEventKind::Done
                };
                emit_event(
                    &callback,
                    LongRunningEvent {
                        task_id,
                        kind,
                        step: last_step,
                        message: None,
                    },
                );
            }

            if let Ok(mut tasks) = task_interrupts().lock() {
                tasks.remove(&task_id);
            }
        });

        Ok(task_id)
    }

    /// Interrupt a task started with [`long_running_start_tsfn()`].
    ///
    /// Returns `true` if interruption was requested successfully.
    #[cfg(feature = "napi")]
    #[napi_derive::napi(js_name = "longRunningCancelTsfn")]
    pub fn long_running_cancel_tsfn(task_id: u32) -> napi::Result<bool> {
        let tasks = task_interrupts().lock().map_err(|_| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "task registry is poisoned".to_string(),
            )
        })?;

        if let Some(flag) = tasks.get(&task_id) {
            flag.store(true, Ordering::Relaxed);
            return Ok(true);
        }

        Ok(false)
    }

    /// Like [`long_running_non_blocking_scoped_thread()`], but uses a regular thread and an owned
    /// cancellation flag so the task can outlive the current stack frame.
    pub fn long_running_non_blocking_thread(
        duration: Duration,
        progres: impl gix::Progress + 'static,
        should_interrupt: Arc<AtomicBool>,
    ) -> std::sync::mpsc::Receiver<anyhow::Result<Data>> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run_long_running_worker(duration, progres, &should_interrupt, tx));
        rx
    }

    fn run_long_running_worker(
        duration: Duration,
        mut progres: impl gix::Progress,
        should_interrupt: &AtomicBool,
        tx: mpsc::Sender<anyhow::Result<Data>>,
    ) {
        const UPDATE_INTERVAL: Duration = Duration::from_millis(100);
        const INTERRUPT_POLL_INTERVAL: Duration = Duration::from_millis(20);

        let total_steps = usize::max(
            1,
            duration
                .as_millis()
                .div_ceil(UPDATE_INTERVAL.as_millis())
                .try_into()
                .unwrap_or(usize::MAX),
        );
        let start = Instant::now();

        progres.init(Some(total_steps), gix::progress::steps());
        progres.set_name("proof of concept task".into());

        for step in 1..=total_steps {
            let scheduled_at = start + duration.mul_f64(step as f64 / total_steps as f64);
            while let Some(remaining) = scheduled_at.checked_duration_since(Instant::now()) {
                if should_interrupt.load(Ordering::Relaxed) {
                    progres.fail(format!("interrupted at step {}/{}", step - 1, total_steps));
                    return;
                }
                thread::sleep(remaining.min(INTERRUPT_POLL_INTERVAL));
            }

            progres.set(step);
            if tx.send(Ok(Data(step))).is_err() {
                progres.info("receiver dropped".into());
                return;
            }
        }

        progres.done("completed".into());
        progres.show_throughput(start);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use super::poc;

    #[test]
    fn long_running_non_blocking_scoped_thread_returns_before_work_completes() {
        let should_interrupt = AtomicBool::new(false);

        thread::scope(|scope| {
            let start = Instant::now();
            let rx = poc::long_running_non_blocking_scoped_thread(
                scope,
                Duration::from_millis(50),
                gix::progress::Discard,
                &should_interrupt,
            );

            assert!(start.elapsed() < Duration::from_millis(25));
            assert!(matches!(
                rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ));

            let values = rx
                .into_iter()
                .collect::<anyhow::Result<Vec<_>>>()
                .expect("proof-of-concept task should complete");

            assert_eq!(values.last().map(|data| data.0), Some(1));
        });
    }

    #[test]
    fn long_running_non_blocking_scoped_thread_stops_when_interrupted() {
        let should_interrupt = AtomicBool::new(false);

        thread::scope(|scope| {
            let rx = poc::long_running_non_blocking_scoped_thread(
                scope,
                Duration::from_millis(200),
                gix::progress::Discard,
                &should_interrupt,
            );
            should_interrupt.store(true, Ordering::Relaxed);

            assert!(matches!(
                rx.recv_timeout(Duration::from_secs(1)),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
            ));
        });
    }

    #[test]
    fn long_running_non_blocking_thread_returns_before_work_completes() {
        let should_interrupt = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let rx = poc::long_running_non_blocking_thread(
            Duration::from_millis(50),
            gix::progress::Discard,
            should_interrupt,
        );

        assert!(start.elapsed() < Duration::from_millis(25));
        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));

        let values = rx
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
            .expect("proof-of-concept task should complete");

        assert_eq!(values.last().map(|data| data.0), Some(1));
    }

    #[test]
    fn long_running_non_blocking_thread_stops_when_interrupted() {
        let should_interrupt = Arc::new(AtomicBool::new(false));
        let rx = poc::long_running_non_blocking_thread(
            Duration::from_millis(200),
            gix::progress::Discard,
            should_interrupt.clone(),
        );
        should_interrupt.store(true, Ordering::Relaxed);

        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(1)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
