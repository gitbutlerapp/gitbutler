#[cfg(any(test, feature = "tokio"))]
pub mod tokio;

/// A resource that is held on an owning thread, and that can be
/// asynchronously locked and interacted with via lambda functions.
///
/// This is used to interact with `git2` resources in a thread-safe
/// manner, since `git2` is not thread-safe nor asynchronous.
pub trait ThreadedResource {
    /// The type of handle returned by [`Self::new`].
    type Handle<T: Unpin + Sized + 'static>: ThreadedResourceHandle<T>;

    /// Creates a new resource; the function passed in will be
    /// executed on the owning thread, the result of which becomes
    /// the owned value that is later interacted with.
    async fn new<T, F, E>(f: F) -> Result<Self::Handle<T>, E>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Unpin + Sized + 'static,
        E: Send + 'static;
}

/// A handle to a resource that is held on an owning thread.
/// This handle can be used to asynchronously lock the resource
/// and interact with it via lambda functions.
///
/// Returned by [`ThreadedResource::new`].
pub trait ThreadedResourceHandle<T: Unpin + Sized + 'static> {
    /// The type of future returned by [`Self::with`].
    type WithFuture<'a, R>: std::future::Future<Output = R> + Send
    where
        Self: 'a,
        R: Send + Unpin + 'static;

    /// Locks the resource, and passes the locked value to the given
    /// function, which can then interact with it. The function is
    /// executed on the owning thread, and the result is returned
    /// to the calling thread asynchronously.
    ///
    /// Note that this is an async-async function - meaning, it
    /// must be awaited in order to receive the future that actually
    /// executes the code, which itself must also be awaited.
    //
    // FIXME(qix-): I think I'm too stupid to understand pinning and phantom
    // FIXME(qix-): data, regardless of how many times I deep-dive into it.
    // FIXME(qix-): I'm now ~48 hours (nearly straight) into this problem,
    // FIXME(qix-): and I've lost a great deal of sanity trying to figure out
    // FIXME(qix-): how to make this work. For now, the async-async function
    // FIXME(qix-): will have to do, but I'm not happy with it. If you know
    // FIXME(qix-): how to make this work, please PLEASE please send a PR.
    // FIXME(qix-): I'm losing sleep and hair over this.
    async fn with<F, R>(&self, f: F) -> Self::WithFuture<'_, R>
    where
        F: FnOnce(&mut T) -> R + Send + Unpin + 'static,
        R: Send + Unpin + 'static;
}
