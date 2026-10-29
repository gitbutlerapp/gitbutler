//! [Tokio](https://tokio.rs) authentication sockets and filesystem checks.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::path::Path;

#[cfg(unix)]
pub use self::unix::TokioAskpassServer;
#[cfg(windows)]
pub use self::windows::TokioAskpassServer;

/// Authentication sockets and helper identity checks using Tokio.
pub struct TokioExecutor;

#[expect(unsafe_code)]
unsafe impl super::GitExecutor for TokioExecutor {
    type Error = std::io::Error;
    type ServerHandle = TokioAskpassServer;

    async unsafe fn create_askpass_server(&self) -> Result<Self::ServerHandle, Self::Error> {
        #[cfg(unix)]
        {
            Self::ServerHandle::new().await
        }
        #[cfg(windows)]
        {
            Self::ServerHandle::new()
        }
    }

    async fn stat<P: AsRef<Path>>(&self, path: P) -> Result<super::FileStat, Self::Error> {
        #[cfg(unix)]
        {
            self::unix::stat(path).await
        }
        #[cfg(windows)]
        {
            self::windows::stat(path).await
        }
    }
}
