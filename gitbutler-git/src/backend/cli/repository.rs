//! NOTE: Doesn't support `no_std` yet.

use std::path::Path;

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
    path: String,
}

impl<E: GitExecutor> Repository<E> {
    /// Opens a repository using the given [`GitExecutor`].
    ///
    /// Note that this **does not** check if the repository exists,
    /// but assumes it does.
    #[inline]
    pub fn open_unchecked<P: AsRef<Path>>(exec: E, path: P) -> Self {
        Self {
            exec,
            path: path.as_ref().to_str().unwrap().to_string(),
        }
    }

    /// (Re-)initializes a repository at the given path
    /// using the given [`GitExecutor`].
    pub async fn open_or_init<P: AsRef<Path>>(exec: E, path: P) -> Result<Self, Error<E::Error>> {
        let path = path.as_ref().to_str().unwrap().to_string();
        let args = vec!["init", "--quiet", &path];

        let (exit_code, stdout, stderr) = exec.execute(&args).await.map_err(Error::Exec)?;

        if exit_code == 0 {
            Ok(Self { exec, path })
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

impl<E: GitExecutor + 'static> crate::Repository for Repository<E> {
    type Error = Error<E::Error>;

    async fn config_get(
        &self,
        key: &str,
        scope: ConfigScope,
    ) -> Result<Option<String>, Self::Error> {
        let mut args = vec!["-C", &self.path, "config", "--get"];

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            #[cfg(not(test))]
            ConfigScope::System => args.push("--system"),
            #[cfg(not(test))]
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
        let mut args = vec!["-C", &self.path, "config", "--replace-all"];

        // NOTE(qix-): See source comments for ConfigScope to explain
        // NOTE(qix-): the `#[cfg(not(test))]` attributes.
        match scope {
            #[cfg(not(test))]
            ConfigScope::Auto => {}
            ConfigScope::Local => args.push("--local"),
            #[cfg(not(test))]
            ConfigScope::System => args.push("--system"),
            #[cfg(not(test))]
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
