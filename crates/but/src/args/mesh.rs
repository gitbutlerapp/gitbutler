//! Arguments for `mesh` and its subcommands.

#![deny(missing_docs)]

/// Every machine and repository of your GitButler account at a glance.
///
/// Lists this machine's repositories and what your other machines published to
/// the hosted server, with who's online, kept live as machines publish. Signs
/// in to GitButler first if needed. The server is `BUT_HOSTED_URL`,
/// https://mesh.but.dev by default.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[clap(args_conflicts_with_subcommands = true)]
pub struct Platform {
    #[clap(subcommand)]
    #[allow(missing_docs)]
    pub cmd: Option<Subcommands>,
    /// Forget the signed-in GitButler account and exit.
    #[clap(long)]
    pub sign_out: bool,
    /// Without a screen: keep this machine online, keep its published branches up to date, pull
    /// branches sent to it, and print what happens, one line each, until interrupted. Needs a
    /// signed-in account.
    #[clap(long)]
    pub headless: bool,
}

#[derive(Debug, clap::Subcommand)]
#[allow(missing_docs)]
pub enum Subcommands {
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Publish(PublishPlatform),
    #[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
    Pull(PullPlatform),
}

/// Publish a branch to the hosted GitButler server.
///
/// Pushes the branch, the target branch it's based on and a snapshot commit in
/// one atomic push. The branch can be checked out in a worktree or applied in
/// the workspace. It's published under this machine's host name, or
/// `BUT_MACHINE`, so it never replaces what other machines published. The
/// server is `BUT_HOSTED_URL`, https://mesh.but.dev by default.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
pub struct PublishPlatform {
    /// The branch to publish, by default the one checked out where you run this.
    pub branch: Option<String>,

    /// Also publish the uncommitted changes of the branch's worktree.
    #[clap(long)]
    pub include_uncommitted: bool,

    /// Also send it to another of your machines, by name, to pull or dismiss there.
    #[clap(long)]
    pub to: Option<String>,
}

/// List the branches published for this project, or pull one down.
///
/// Without a branch, lists what each of your other machines published. With
/// one, pulls it from the machine that published it (`--from` when several
/// did): it updates the
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

    /// The machine to pull it from, needed when more than one published it.
    #[clap(long)]
    pub from: Option<String>,

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
