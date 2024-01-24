//! A [Tokio](https://tokio.rs)-based [`GitExecutor`] implementation.

use crate::prelude::*;
use tokio::process::Command;

/// A [`GitExecutor`] implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;

    async fn execute_raw(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let mut cmd = Command::new("git");
        cmd.args(args);
        if let Some(envs) = envs {
            cmd.envs(envs);
        }

        let output = cmd.output().await?;

        Ok((
            output.status.code().unwrap_or(127) as usize,
            String::from_utf8_lossy(&output.stdout).trim().into(),
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ))
    }
}
