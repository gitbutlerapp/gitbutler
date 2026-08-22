use std::path::PathBuf;

/// Create linked git worktrees that GitButler recognizes.
///
/// A worktree created here is an ordinary git worktree — `but status` discovers it through git,
/// so nothing else is needed to make it visible.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// The subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// The `but worktree` subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Create a worktree at `path`, checked out at the workspace's base commit.
    ///
    /// With `--cow`, the worktree is populated by cloning the current working directory
    /// copy-on-write instead of checking every file out. On a filesystem that supports it
    /// (APFS, btrfs, XFS with reflinks) the clone is near-instant and costs almost no disk,
    /// and it carries untracked build output — `target/`, `node_modules/` — across with it,
    /// so builds in the new worktree start warm.
    New {
        /// Where to create the worktree.
        path: PathBuf,
        /// Populate the worktree with a copy-on-write clone of the current working directory.
        ///
        /// Falls back to a normal checkout, with a notice, when the filesystem cannot clone.
        #[clap(short = 'c', long = "cow")]
        cow: bool,
    },
}
