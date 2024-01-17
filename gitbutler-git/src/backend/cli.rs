//! CLI-based (fork/exec) backend implementation,
//! executing the `git` command-line tool available
//! on `$PATH`.

mod executor;
mod repository;

pub use self::{executor::GitExecutor, repository::Repository};

#[cfg(feature = "tokio")]
pub use self::executor::tokio;
