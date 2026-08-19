use anyhow::Context as _;
use bstr::BStr;

/// The branch checked out in the linked worktree `name`, which is where a commit made from that
/// checkout goes and where a commit moved onto its lane lands.
///
/// A detached worktree has no branch to move, so it is refused rather than silently targeting
/// somewhere else. The same goes for a workspace ref: such worktrees never render a heading,
/// but the checkout can change between render and confirm, and a workspace ref must never
/// receive a commit meant for a branch.
pub(crate) fn worktree_branch(
    repo: &gix::Repository,
    name: &BStr,
) -> anyhow::Result<gix::refs::FullName> {
    let worktree_repo = but_workspace::worktrees::open_worktree_repo(repo, name)?;
    let branch = worktree_repo.head_name()?.with_context(|| {
        format!("Worktree {name} has a detached HEAD, so there is no branch to commit to")
    })?;
    anyhow::ensure!(
        !but_core::is_workspace_ref_name(branch.as_ref()),
        "Worktree {name} has a workspace ref checked out, so there is no branch to commit to"
    );
    Ok(branch)
}
