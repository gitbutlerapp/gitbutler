//! [libgit2](https://libgit2.org/) implementation of
//! the core `gitbutler-git` library traits.
//!
//! The entry point for this module is the [`Repository`] struct.

mod repository;

pub use self::repository::Repository;
