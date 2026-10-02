//! Arguments for `_publish` and `_pull`.

#![deny(missing_docs)]

/// Publish a branch to the hosted GitButler server.
///
/// Pushes the branch, the target branch it's based on and a snapshot commit in
/// one atomic push. The branch can be checked out in a worktree or applied in
/// the workspace. Branches are keyed by name: if the published one has commits
/// this one doesn't, it asks before replacing them in a terminal, and otherwise
/// needs `--overwrite` or `--keep`. The server is `BUT_HOSTED_URL` with
/// `BUT_HOSTED_TOKEN`, a local demo server by default.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct Platform {
    /// The branch to publish, by default the one checked out where you run this.
    pub branch: Option<String>,

    /// Also publish the uncommitted changes of the branch's worktree.
    #[clap(long)]
    pub include_uncommitted: bool,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub conflict: Conflict,
}

/// List the branches published for this project, or pull one down.
///
/// Without a branch, lists what the hosted server has. With one, updates the
/// local branch where it lives, or else adds a worktree for it next to the main
/// one, with its published uncommitted changes, or applies it in the workspace
/// with `--into-workspace`. If the local branch has work that isn't published,
/// it asks before replacing it in a terminal, and otherwise needs `--overwrite`
/// or `--keep`.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct PullPlatform {
    /// The published branch to pull down.
    pub branch: Option<String>,

    /// Apply it in the workspace instead of adding a worktree, if it's not local yet.
    #[clap(long)]
    pub into_workspace: bool,

    #[clap(flatten)]
    #[allow(missing_docs)]
    pub conflict: Conflict,
}

/// What to do when work would be replaced.
#[derive(Debug, clap::Args)]
pub struct Conflict {
    /// Replace work that would be lost, without asking.
    #[clap(long, group = "conflict")]
    pub overwrite: bool,

    /// Keep work that would be lost and do nothing, without asking.
    #[clap(long, group = "conflict")]
    pub keep: bool,
}
