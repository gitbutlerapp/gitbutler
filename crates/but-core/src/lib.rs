#![deny(missing_docs, rust_2018_idioms)]
//! The basic primitives that GitButler is built around.
//!
//! It also is a catch-all for code until it's worth putting it into its own crate.
//!
//! ### House-~~Rules~~ Guidance
//!
//! * Try hard to do write all the 'right' tests
//!    - Tests should challenge the implementation, try hard to break it.
//!    - capture *all* business requirements
//!    - Try to avoid doing read-only filesystem fixtures with `tempdir`, instead use `gitbutler-testtools::readonly`.
//! * minimal dependencies
//!    - both for the crate and for parameters of functions as well.
//!         - i.e. try to avoid 'God' structures so the function only has access to what it needs to.
//! * The filesystem is `Sync` but we don't have atomic operations
//!    - Let's be very careful about changes to the filesystem, must at least be on the level of Git which means `.lock` files instead of direct writes.
//!    - If only one part of the application is supposed to change the worktree, let's protect the Application from itself by using `gitbutler::access` just like we do now.
//! * Make it work, make it work right, and if time and profiler permits, make it work fast.
//! * All of the above can and should be scrutinized and is there is no hard rules.

/// Functions related to a Git worktree, i.e. the files checked out from a repository.
pub mod worktree {
    use std::path::Path;

    /// Return a list of items that live underneath `worktree_root` that changed and thus can become part of a commit.
    pub fn committable_entries(_worktree_root: &Path) -> anyhow::Result<()> {
        todo!()
    }
}
