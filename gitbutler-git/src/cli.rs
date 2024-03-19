//! CLI-based (fork/exec) backend implementation,
//! executing the `git` command-line tool available
//! on `$PATH`.

mod executor;
mod repository;

#[cfg(unix)]
pub use self::executor::Uid;

pub use self::{
    executor::{AskpassServer, FileStat, GitExecutor, Pid, Socket},
    repository::fetch,
};

#[cfg(feature = "tokio")]
pub use self::executor::tokio;
