use crate::executor::{AskpassServer, FileStat, Pid, Socket};
use std::{
    cell::RefCell,
    os::windows::{fs::MetadataExt, io::AsRawHandle},
    path::Path,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::Mutex,
};

const ASKPASS_PIPE_PREFIX: &str = r"\\.\pipe\gitbutler-askpass-";

impl Socket for BufStream<NamedPipeServer> {
    type Error = std::io::Error;

    fn pid(&self) -> Result<Pid, Self::Error> {
        let raw_handle = self.get_ref().as_raw_handle();
        let mut out_pid: winapi::shared::minwindef::ULONG = 0;

        #[allow(unsafe_code)]
        let r = unsafe {
            winapi::um::winbase::GetNamedPipeClientProcessId(
                // We need the `as` here to make rustdoc shut up
                // about winapi using different type defs for docs.
                raw_handle as winapi::um::winnt::HANDLE,
                &mut out_pid,
            )
        };

        if r == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Pid::from(out_pid))
        }
    }

    async fn read_line(&mut self) -> Result<String, Self::Error> {
        let mut buf = String::new();
        <Self as AsyncBufReadExt>::read_line(self, &mut buf).await?;
        Ok(buf.trim_end_matches(|c| c == '\r' || c == '\n').into())
    }

    async fn write_line(&mut self, line: &str) -> Result<(), Self::Error> {
        <Self as AsyncWriteExt>::write_all(self, line.as_bytes()).await?;
        <Self as AsyncWriteExt>::write_all(self, b"\n").await?;
        <Self as AsyncWriteExt>::flush(self).await?;
        Ok(())
    }
}

/// A server for the `askpass` protocol using Tokio.
pub struct TokioAskpassServer {
    server: Mutex<RefCell<NamedPipeServer>>,
    connection_string: String,
}

impl TokioAskpassServer {
    pub(crate) fn new() -> Result<Self, std::io::Error> {
        let connection_string = format!("{ASKPASS_PIPE_PREFIX}{}", rand::random::<u64>());

        let server = Mutex::new(RefCell::new(
            ServerOptions::new()
                .first_pipe_instance(true)
                .create(&connection_string)?,
        ));

        Ok(TokioAskpassServer {
            server,
            connection_string,
        })
    }
}

impl AskpassServer for TokioAskpassServer {
    type Error = std::io::Error;
    type SocketHandle = BufStream<NamedPipeServer>;

    // We can ignore clippy here since we locked the mutex.
    #[allow(clippy::await_holding_refcell_ref)]
    async fn accept(&self, timeout: Option<Duration>) -> Result<Self::SocketHandle, Self::Error> {
        let server = self.server.lock().await;

        if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, server.borrow().connect()).await??;
        } else {
            server.borrow().connect().await?;
        }

        // Windows is weird. The server becomes the peer connection,
        // and before we use the new connection, we first create
        // a new server to listen for the next connection.
        let client = server.replace(ServerOptions::new().create(&self.connection_string)?);

        Ok(tokio::io::BufStream::new(client))
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
        // Best effort
        let _ = self.server.get_mut().get_mut().disconnect();
    }
}

pub async fn stat<P: AsRef<Path>>(path: P) -> Result<FileStat, std::io::Error> {
    let metadata = tokio::fs::symlink_metadata(path).await?;

    // NOTE(qix-): We can safely unwrap here since the docs say:
    // NOTE(qix-):
    // NOTE(qix-): > This will return `None`` if the Metadata instance was created
    // NOTE(qix-): > from a call to `DirEntry::metadata`. If this `Metadata` was created
    // NOTE(qix-): > by using `fs::metadata` or `File::metadata`, then this will return `Some`.
    // NOTE(qix-):
    // NOTE(qix-): Thus, since we're not using directory entries, these are guaranteed to
    // NOTE(qix-): return `Some`.
    Ok(FileStat {
        dev: metadata.volume_serial_number().unwrap().into(),
        ino: metadata.file_index().unwrap(),
        is_regular_file: metadata.is_file(),
    })
}
