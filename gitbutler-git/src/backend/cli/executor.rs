#[cfg(feature = "tokio")]
pub mod tokio;

/// Provides a means for executing Git CLI commands.
///
/// There is no `arg0` passed; it's up to the implementation
/// to decide how to execute the command. For example,
/// `git status` would be passed as `["status"]`.
pub trait GitExecutor {
    /// The error type returned by this executor,
    /// specifically in cases where the execution fails.
    ///
    /// Otherwise, `Ok` is returned in call cases, even when
    /// the exit code is non-zero.
    type Error: core::error::Error + core::fmt::Debug + Send + Sync + 'static;

    /// Executes the given Git command with the given arguments.
    /// `git` is never passed as the first argument (arg 0).
    ///
    /// Returns a tuple of `(exit_code, stdout, stderr)`.
    ///
    /// `Err` is returned if the command could not be executed,
    /// **not** if the command returned a non-zero exit code.
    async fn execute(&self, args: &[&str]) -> Result<(usize, String, String), Self::Error>;
}
