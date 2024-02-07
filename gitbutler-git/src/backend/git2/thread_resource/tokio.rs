//! A [Tokio](https://tokio.rs)-based implementation for [libgit2](https://libgit2.org/)
//! repository backends, allowing normally blocking libgit2 operations to be run on a
//! threadpool, asynchronously.

use futures::Future;
use std::{
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Barrier, Mutex as SyncMutex},
    task::{Context, Poll, Waker},
    thread::{JoinHandle, Thread},
};
use tokio::sync::Mutex as AsyncMutex;

/// A [`super::ThreadedResource`] implementation using Tokio.
pub struct TokioThreadedResource;

/// A [`super::ThreadedResourceHandle`] implementation using Tokio.
pub struct TokioThreadedResourceHandle<T: Unpin + Sized + 'static> {
    terminate: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    access_control_mutex: Arc<AsyncMutex<()>>,
    #[allow(clippy::type_complexity)]
    slot: Arc<SyncMutex<Option<(Waker, Box<dyn FnOnce(&mut T) + Send>)>>>,
}

impl super::ThreadedResource for TokioThreadedResource {
    type Handle<T: Unpin + Sized + 'static> = TokioThreadedResourceHandle<T>;

    async fn new<T, F, E>(f: F) -> Result<Self::Handle<T>, E>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Unpin + Sized + 'static,
        E: Send + 'static,
    {
        #[allow(clippy::type_complexity)]
        let slot: Arc<SyncMutex<Option<(Waker, Box<dyn FnOnce(&mut T) + Send>)>>> =
            Arc::new(SyncMutex::new(None));

        let maybe_error = Arc::new(SyncMutex::new(None));
        let barrier = Arc::new(Barrier::new(2));

        let terminate_signal = Arc::new(AtomicBool::new(false));

        let thread = std::thread::spawn({
            let slot = Arc::clone(&slot);
            let barrier = Arc::clone(&barrier);
            let maybe_error = Arc::clone(&maybe_error);
            let terminate_signal = Arc::clone(&terminate_signal);
            move || {
                let mut v = match f() {
                    Ok(v) => v,
                    Err(e) => {
                        *maybe_error.lock().unwrap() = Some(e);
                        barrier.wait();
                        return;
                    }
                };

                barrier.wait();

                loop {
                    if terminate_signal.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    std::thread::park();
                    if terminate_signal.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }

                    if let Some((waker, fun)) = slot.lock().unwrap().take() {
                        fun(&mut v);
                        waker.wake();
                    } else {
                        break;
                    }
                }
            }
        });

        barrier.wait();

        if let Some(e) = maybe_error.lock().unwrap().take() {
            return Err(e);
        }

        Ok(TokioThreadedResourceHandle {
            thread,
            slot,
            access_control_mutex: Arc::new(AsyncMutex::new(())),
            terminate: terminate_signal,
        })
    }
}

impl<T> Drop for TokioThreadedResourceHandle<T>
where
    T: Unpin + Sized + 'static,
{
    fn drop(&mut self) {
        self.terminate
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.thread.thread().unpark();
    }
}

impl<T> super::ThreadedResourceHandle<T> for TokioThreadedResourceHandle<T>
where
    T: Unpin + Sized + 'static,
{
    type WithFuture<'a, R> = impl Future<Output = R> + Send
    where
        Self: 'a,
        R: Send + Unpin + 'static;

    async fn with<F, R>(&self, f: F) -> Self::WithFuture<'_, R>
    where
        F: FnOnce(&mut T) -> R + Send + Unpin + 'static,
        R: Send + Unpin + 'static,
    {
        let guard = self.access_control_mutex.lock().await;

        let result_slot = Arc::new(SyncMutex::new(Option::<R>::None));
        let result_slot_clone = Arc::clone(&result_slot);
        let slot = Arc::clone(&self.slot);

        let boxed_f = Box::new(move |v: &mut T| {
            *result_slot.lock().unwrap() = Some(f(v));
        });

        TokioThreadedResourceHandleFuture {
            set_fun: Some(Box::new(move |waker| {
                slot.lock().unwrap().replace((waker, boxed_f));
            })),
            result_slot: result_slot_clone,
            handle: self.thread.thread(),
            _access_guard: guard,
        }
    }
}

/// The future returned by [`TokioThreadedResourceHandle`]::with.
pub struct TokioThreadedResourceHandleFuture<'thread, R, Guard>
where
    R: Send + Unpin + 'static,
    Guard: Unpin,
{
    set_fun: Option<Box<dyn FnOnce(Waker) + Send + Unpin + 'static>>,
    result_slot: Arc<SyncMutex<Option<R>>>,
    _access_guard: Guard,
    handle: &'thread Thread,
}

impl<'thread, R, Guard> Future for TokioThreadedResourceHandleFuture<'thread, R, Guard>
where
    R: Send + Unpin + 'static,
    Guard: Unpin,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<R> {
        let this = self.as_mut().get_mut();

        if let Some(set_fun) = this.set_fun.take() {
            set_fun(cx.waker().clone());
            this.handle.unpark();
            return Poll::Pending;
        }

        if let Ok(mut result_slot) = this.result_slot.try_lock() {
            if let Some(result) = result_slot.take() {
                return Poll::Ready(result);
            }
        }

        Poll::Pending
    }
}
