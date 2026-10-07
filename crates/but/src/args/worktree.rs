use crate::args::atoms::{BranchArg, CliIdArg};

/// How to populate a new worktree.
#[cfg(feature = "worktree-cow")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CreateMode {
    /// Clone files from the main worktree, including ignored files. This is useful to speed up
    /// builds with build tools that cache artifacts and dependencies in the working directory, as
    /// well as reduce overall disk space footprint.
    Cow,
    /// Check out tracked files, then clone ignored artifacts using destination ignore rules.
    CowIgnored,
    /// Check out tracked files, then clone only the root target directory without overwriting tracked files.
    CowTarget,
    /// Perform a standard checkout in the new worktree. Only tracked files carry over.
    Checkout,
}

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
    /// Create a worktree on a new branch at the workspace base or above a commit.
    ///
    /// By default, the branch starts at the child-most commit any applied stack rests on, and is
    /// checked out under `~/.gitbutler-worktrees/<repo-dir-basename>/` in a directory
    /// named by a slug of the branch name.
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    New {
        /// Start the new branch at COMMIT instead of the workspace base.
        ///
        /// Accepts a commit SHA or CLI ID, not a branch. Conflicted commits are refused.
        #[clap(short = 'A', long, value_name = "COMMIT")]
        above: Option<CliIdArg>,
        /// The name of the branch to create, or a generated one.
        name: Option<BranchArg>,
        /// Select how to populate the worktree.
        #[cfg(feature = "worktree-cow")]
        #[clap(long, value_enum, default_value = "checkout")]
        create_mode: CreateMode,
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
