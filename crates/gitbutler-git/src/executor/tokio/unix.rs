use std::{
    fs::Permissions,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::{UnixListener, UnixStream},
};

use crate::executor::{AskpassServer, FileStat, Pid, Socket, Uid};

impl Socket for BufStream<UnixStream> {
    type Error = std::io::Error;

    fn pid(&self) -> Result<Pid, Self::Error> {
        self.get_ref()
            .peer_cred()
            .unwrap()
            .pid()
            .ok_or(std::io::Error::other(
                "no pid available for peer connection",
            ))
    }

    fn uid(&self) -> Result<Uid, Self::Error> {
        Ok(self.get_ref().peer_cred().unwrap().uid())
    }

    async fn read_line(&mut self) -> Result<String, Self::Error> {
        let mut buf = String::new();
        <Self as AsyncBufReadExt>::read_line(self, &mut buf).await?;
        // TODO: use an array of `char`
        #[expect(clippy::manual_pattern_char_comparison)]
        Ok(buf.trim_end_matches(|c| c == '\r' || c == '\n').into())
    }

    async fn write_line(&mut self, line: &str) -> Result<(), Self::Error> {
        <Self as AsyncWriteExt>::write_all(self, line.as_bytes()).await?;
        <Self as AsyncWriteExt>::write_all(self, b"\n").await?;
        <Self as AsyncWriteExt>::flush(self).await?;
        Ok(())
    }
}

/// A tokio-based askpass server implementation.
pub struct TokioAskpassServer {
    // Always Some until dropped.
    server: Option<UnixListener>,
    connection_string: String,
}

impl TokioAskpassServer {
    pub(crate) async fn new() -> Result<Self, std::io::Error> {
        let connection_string =
            std::env::temp_dir().join(format!("gitbutler-askpass-{}", rand::random::<u64>()));

        let listener = UnixListener::bind(&connection_string)?;

        tokio::fs::set_permissions(&connection_string, Permissions::from_mode(0o0600)).await?;

        Ok(TokioAskpassServer {
            server: Some(listener),
            connection_string: connection_string.to_string_lossy().into(),
        })
    }
}

impl AskpassServer for TokioAskpassServer {
    type Error = std::io::Error;
    type SocketHandle = BufStream<UnixStream>;

    async fn accept(&self, timeout: Option<Duration>) -> Result<Self::SocketHandle, Self::Error> {
        let res = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, self.server.as_ref().unwrap().accept()).await?
        } else {
            self.server.as_ref().unwrap().accept().await
        };

        res.map(|(s, _)| BufStream::new(s))
    }
}

impl core::fmt::Display for TokioAskpassServer {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.connection_string.fmt(f)
    }
}

impl Drop for TokioAskpassServer {
    fn drop(&mut self) {
        drop(self.server.take());
        // best-effort
        std::fs::remove_file(&self.connection_string).ok();
    }
}

pub async fn stat<P: AsRef<Path>>(path: P) -> Result<FileStat, std::io::Error> {
    let metadata = tokio::fs::symlink_metadata(path).await?;

    Ok(FileStat {
        dev: metadata.dev(),
        ino: metadata.ino(),
        is_regular_file: metadata.is_file(),
    })
}
