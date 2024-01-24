use crate::prelude::*;

#[cfg(any(test, feature = "tokio"))]
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
    async fn execute_raw(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error>;

    /// Executes the given Git command with sane defaults.
    /// `git` is never passed as the first argument (arg 0).
    ///
    /// Implementers should use this method over [`Self::execute_raw`]
    /// when possible.
    async fn execute(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let mut args = args.as_ref().to_vec();

        args.insert(0, "--no-pager");
        // TODO(qix-): Test the performance impact of this.
        args.insert(0, "--no-optional-locks");

        let mut envs = envs.unwrap_or_default();
        envs.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        envs.insert("LC_ALL".into(), "C".into()); // Force English. We need this for parsing output.

        self.execute_raw(&args, Some(envs)).await
    }
}
