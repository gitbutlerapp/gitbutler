use crate::args::atoms::{BranchArg, CliIdArg};

/// Manage worktrees (experimental, requires the `worktreeManipulation` feature flag).
///
/// Without a subcommand, lists the worktrees.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Option<Subcommands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// List worktrees, most recently updated first.
    ///
    /// By default this lists every active worktree and the three most recently updated
    /// archived ones. A worktree is shown by its name, followed by the branch it has checked
    /// out when that differs from the name, and its path.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    List {
        /// List all archived worktrees.
        #[clap(long)]
        archived: bool,
        /// List all active worktrees.
        #[clap(long)]
        active: bool,
    },
    /// Create a worktree on a new branch at the workspace base.
    ///
    /// By default the branch starts at the child-most commit any applied stack rests on,
    /// and is checked out under `.git/gb-wts/` in a directory named by a slug of the branch
    /// name. With `--cow`, it instead starts at the primary worktree's HEAD and clones its
    /// current files and index, even when invoked from a linked worktree.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    New {
        /// The name of the branch to create, or a generated one.
        name: Option<BranchArg>,
        /// Clone the primary worktree at its HEAD using copy-on-write (macOS only).
        ///
        /// Includes ignored and untracked files and preserves staged/unstaged changes.
        /// Requires a filesystem supporting clonefile(2); no full-copy fallback.
        /// Avoid concurrent edits while cloning: this is not an atomic snapshot.
        #[clap(long)]
        cow: bool,
    },
    /// Hide a worktree from the workspace.
    Archive {
        /// The worktree, by CLI ID (see `but wt list`) or name.
        worktree: CliIdArg,
    },
    /// Show an archived worktree in the workspace again.
    Unarchive {
        /// The worktree, by CLI ID (see `but wt list`) or name.
        worktree: CliIdArg,
    },
    /// Remove a worktree from disk, like `git worktree remove`.
    ///
    /// This works on archived worktrees too, and keeps the branch the worktree had checked out.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    #[clap(visible_alias = "rm")]
    Remove {
        /// Remove the worktree even if it has uncommitted changes.
        #[clap(short, long)]
        force: bool,
        /// The worktree, by CLI ID (see `but wt list`) or name.
        worktree: CliIdArg,
    },
}
