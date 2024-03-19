//! A [Tokio](https://tokio.rs)-based [`super::GitExecutor`] implementation.

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{collections::HashMap, fs::Permissions, path::Path, time::Duration};
use tokio::process::Command;

/// A [`super::GitExecutor`] implementation using the `git` command-line tool
/// via [`tokio::process::Command`].
pub struct TokioExecutor;

#[allow(unsafe_code)]
unsafe impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;
    type ServerHandle = TokioAskpassServer;

    async fn execute_raw<P: AsRef<Path>>(
        &self,
        args: &[&str],
        cwd: P,
        envs: Option<HashMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let mut cmd = Command::new("git");

        // Output the command being executed to stderr, for debugging purposes
        // (only on test configs).
        #[cfg(any(test, debug_assertions))]
        {
            let mut envs_str = String::new();
            if let Some(envs) = &envs {
                for (key, value) in envs.iter() {
                    envs_str.push_str(&format!("{}={} ", key, value));
                }
            }
            let args_str = args.join(" ");
            eprintln!("env {envs_str} git {args_str}");
        }

        cmd.kill_on_drop(true);
        cmd.args(args);
        cmd.current_dir(cwd);

        if let Some(envs) = envs {
            cmd.envs(envs);
        }

        let output = cmd.output().await?;

        #[cfg(any(test, debug_assertions))]
        {
            eprintln!(
                "\n\n GIT STDOUT:\n\n{}\n\nGIT STDERR:\n\n{}\n\nGIT EXIT CODE: {}\n",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
                output.status.code().unwrap_or(127) as usize
            );
        }

        Ok((
            output.status.code().unwrap_or(127) as usize,
            String::from_utf8_lossy(&output.stdout).trim().into(),
            String::from_utf8_lossy(&output.stderr).trim().into(),
        ))
    }

    #[cfg(unix)]
    async unsafe fn create_askpass_server(&self) -> Result<Self::ServerHandle, Self::Error> {
        let connection_string =
            std::env::temp_dir().join(format!("gitbutler-askpass-{}", rand::random::<u64>()));

        let listener = tokio::net::UnixListener::bind(&connection_string)?;

        tokio::fs::set_permissions(&connection_string, Permissions::from_mode(0o0600)).await?;

        Ok(TokioAskpassServer {
            server: Some(listener),
            connection_string: connection_string.to_string_lossy().into(),
        })
    }

    #[cfg(unix)]
    async fn stat(&self, path: &str) -> Result<super::FileStat, Self::Error> {
        let metadata = tokio::fs::symlink_metadata(path).await?;

        Ok(super::FileStat {
            dev: metadata.dev(),
            ino: metadata.ino(),
            is_regular_file: metadata.is_file(),
        })
    }
}

#[cfg(unix)]
impl super::Socket for tokio::io::BufStream<tokio::net::UnixStream> {
    type Error = std::io::Error;

    fn pid(&self) -> Result<super::Pid, Self::Error> {
        self.get_ref()
            .peer_cred()
            .unwrap()
            .pid()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no pid available for peer connection",
            ))
    }

    fn uid(&self) -> Result<super::Uid, Self::Error> {
        Ok(self.get_ref().peer_cred().unwrap().uid())
    }

    async fn read_line(&mut self) -> Result<String, Self::Error> {
        let mut buf = String::new();
        <Self as tokio::io::AsyncBufReadExt>::read_line(self, &mut buf).await?;
        Ok(buf.trim_end_matches(|c| c == '\r' || c == '\n').into())
    }

    async fn write_line(&mut self, line: &str) -> Result<(), Self::Error> {
        <Self as tokio::io::AsyncWriteExt>::write_all(self, line.as_bytes()).await?;
        <Self as tokio::io::AsyncWriteExt>::write_all(self, b"\n").await?;
        <Self as tokio::io::AsyncWriteExt>::flush(self).await?;
        Ok(())
    }
}

/// A tokio-based [`super::AskpassServer`] implementation.
#[cfg(unix)]
pub struct TokioAskpassServer {
    // Always Some until dropped.
    server: Option<tokio::net::UnixListener>,
    connection_string: String,
}

#[cfg(unix)]
impl super::AskpassServer for TokioAskpassServer {
    type Error = std::io::Error;
    #[cfg(unix)]
    type SocketHandle = tokio::io::BufStream<tokio::net::UnixStream>;

    async fn accept(&self, timeout: Option<Duration>) -> Result<Self::SocketHandle, Self::Error> {
        let res = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, self.server.as_ref().unwrap().accept()).await?
        } else {
            self.server.as_ref().unwrap().accept().await
        };

        res.map(|(s, _)| tokio::io::BufStream::new(s))
    }
}

#[cfg(unix)]
impl core::fmt::Display for TokioAskpassServer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.connection_string.fmt(f)
    }
}

#[cfg(unix)]
impl Drop for TokioAskpassServer {
    fn drop(&mut self) {
        drop(self.server.take());
        // best-effort
        std::fs::remove_file(&self.connection_string).ok();
    }
}
