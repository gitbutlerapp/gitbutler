//! NOTE: Doesn't support `no_std` yet.

use super::executor::GitExecutor;
use crate::ConfigScope;

/// Higher level errors that can occur when interacting with the CLI.
#[derive(Debug, thiserror::Error)]
pub enum Error<E: core::error::Error + core::fmt::Debug + Send + Sync + 'static> {
    #[error("failed to execute git command: {0}")]
    Exec(E),
    #[error(
        "git command exited with non-zero exit code {0}: {1:?}\n\nSTDOUT:\n{2}\n\nSTDERR:\n{3}"
    )]
    Failed(usize, Vec<String>, String, String),
}

/// A [`crate::Repository`] implementation using the `git` CLI
/// and the given [`GitExecutor`] implementation.
pub struct Repository<E: GitExecutor> {
    exec: E,
}

impl<E: GitExecutor> Repository<E> {
    /// Creates a new repository using the given [`GitExecutor`].
    #[inline]
    pub fn new(exec: E) -> Self {
        Self { exec }
    }
}

impl<E: GitExecutor + 'static> crate::Repository for Repository<E> {
    type Error = Error<E::Error>;

    async fn config_get(
        &self,
        key: &str,
        scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error> {
        let mut args = vec!["config", "--get"];
        match scope {
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            ConfigScope::System => args.push("--system"),
            ConfigScope::Global => args.push("--global"),
        }
        args.push(key);

        let (exit_code, stdout, stderr) = self.exec.execute(&args).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(Some(stdout))
        } else if exit_code == 1 && stderr.is_empty() {
            Ok(None)
        } else {
            Err(Error::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }

    async fn config_set(
        &self,
        key: &str,
        value: &str,
        scope: ConfigScope,
    ) -> Result<(), Self::Error> {
        let mut args = vec!["config", "--set"];
        match scope {
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            ConfigScope::System => args.push("--system"),
            ConfigScope::Global => args.push("--global"),
        }
        args.push(key);
        args.push(value);

        let (exit_code, stdout, stderr) = self.exec.execute(&args).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(())
        } else {
            Err(Error::Failed(
                exit_code,
                args.into_iter().map(Into::into).collect(),
                stdout,
                stderr,
            ))
        }
    }
}
