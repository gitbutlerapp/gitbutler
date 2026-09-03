use crate::args::atoms::CliIdArg;

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
    Remove {
        /// Remove the worktree even if it has uncommitted changes.
        #[clap(short, long)]
        force: bool,
        /// The worktree, by CLI ID (see `but wt list`) or name.
        worktree: CliIdArg,
    },
}
