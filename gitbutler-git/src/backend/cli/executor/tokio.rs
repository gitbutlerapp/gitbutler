//! A [Tokio](https://tokio.rs)-based [`GitExecutor`] implementation.

use std::collections::HashMap;
use tokio::process::Command;

/// A [`GitExecutor`] implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;

    async fn execute(&self, args: &[&str]) -> Result<(usize, String, String), Self::Error> {
        let mut cmd = Command::new("git");
        cmd.args(args);

        let output = cmd.output().await?;

        Ok((
            output.status.code().unwrap_or(127) as usize,
            String::from_utf8_lossy(&output.stdout).trim().into(),
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ))
    }
}

/// A [`GitExecutor`] implementation using the `git` command-line tool
/// via [`tokio::process::Command`], with the given environment variables.
pub struct TokioExecutorEnv {
    env: HashMap<String, String>,
}

impl super::GitExecutor for TokioExecutorEnv {
    type Error = std::io::Error;

    async fn execute(&self, args: &[&str]) -> Result<(usize, String, String), Self::Error> {
        let mut cmd = Command::new("git");
        cmd.args(args);
        cmd.envs(&self.env);

        let output = cmd.output().await?;

        Ok((
            output.status.code().unwrap_or(127) as usize,
            String::from_utf8_lossy(&output.stdout).trim().into(),
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ))
    }
}

/// Allows executors to create (or modify) a [`TokioExecutorEnv`],
/// with added/modified environment variables, set for each execution
/// of `git`.
pub trait WithEnv: Sized {
    /// Sets the given environment variable.
    fn with_env<K: AsRef<str>, V: AsRef<str>>(self, key: K, value: V) -> TokioExecutorEnv;

    /// Creates a new [`TokioExecutorEnv`] with the given additional environment variables.
    fn with_envs<K: AsRef<str>, V: AsRef<str>, I: IntoIterator<Item = (K, V)>>(
        self,
        envs: I,
    ) -> TokioExecutorEnv;
}

impl WithEnv for TokioExecutor {
    fn with_env<K: AsRef<str>, V: AsRef<str>>(self, key: K, value: V) -> TokioExecutorEnv {
        TokioExecutorEnv {
            env: [(key.as_ref().into(), value.as_ref().into())]
                .iter()
                .cloned()
                .collect(),
        }
    }

    fn with_envs<K: AsRef<str>, V: AsRef<str>, I: IntoIterator<Item = (K, V)>>(
        self,
        envs: I,
    ) -> TokioExecutorEnv {
        TokioExecutorEnv {
            env: envs
                .into_iter()
                .map(|(k, v)| (k.as_ref().into(), v.as_ref().into()))
                .collect(),
        }
    }
}

impl WithEnv for TokioExecutorEnv {
    fn with_env<K: AsRef<str>, V: AsRef<str>>(mut self, key: K, value: V) -> TokioExecutorEnv {
        self.env.insert(key.as_ref().into(), value.as_ref().into());
        self
    }

    fn with_envs<K: AsRef<str>, V: AsRef<str>, I: IntoIterator<Item = (K, V)>>(
        mut self,
        envs: I,
    ) -> TokioExecutorEnv {
        self.env.extend(
            envs.into_iter()
                .map(|(k, v)| (k.as_ref().into(), v.as_ref().into())),
        );
        self
    }
}
