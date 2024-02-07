#[cfg(feature = "cli")]
pub mod cli;

// We use the libgit2 backend for tests as well.
#[cfg(any(test, feature = "git2"))]
pub mod git2;
