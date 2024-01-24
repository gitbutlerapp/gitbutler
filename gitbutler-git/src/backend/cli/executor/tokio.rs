//! A [Tokio](https://tokio.rs)-based [`GitExecutor`] implementation.

use crate::prelude::*;
use tokio::process::Command;

/// A [`GitExecutor`] implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;
    #[cfg(unix)]
    type SocketHandle = tokio::io::BufStream<tokio::net::UnixStream>;

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

#[cfg(unix)]
impl super::Socket for tokio::io::BufStream<tokio::net::UnixStream> {
    type Error = std::io::Error;

    fn pid(&self) -> Result<super::Pid, Self::Error> {
        self.peer_cred().unwrap().pid()
    }

    fn uid(&self) -> Result<super::Uid, Self::Error> {
        self.peer_cred().unwrap().uid()
    }

    async fn read_line(&mut self) -> Result<String, Self::Error> {
        let mut buf = String::new();
        <Self as tokio::io::AsyncBufReadExt>::read_line(self, &mut buf).await?;
        Ok(buf)
    }

    async fn write_line(&mut self, line: &str) -> Result<(), Self::Error> {
        <Self as tokio::io::AsyncWriteExt>::write_all(self, line.as_bytes()).await?;
        <Self as tokio::io::AsyncWriteExt>::write_all(self, b"\n").await?;
        Ok(())
    }
}

#[cfg(unix)]
pub struct TokioAskpassServer {
    abort_handle: tokio::sync::AbortHandle,
    pathname: String,
}

#[cfg(unix)]
impl super::AskpassServer for TokioAskpassServer {}

#[cfg(unix)]
impl core::fmt::Display for TokioAskpassServer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.pathname.fmt(f)
    }
}

#[cfg(unix)]
impl Drop for TokioAskpassServer {
    fn drop(&mut self) {
        self.abort_handle.abort();
        // best-effort
        std::fs::remove_file(&self.pathname).ok();
    }
}
