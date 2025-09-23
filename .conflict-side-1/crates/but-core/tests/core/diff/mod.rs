use anyhow::Context;
use but_core::{TreeChange, UnifiedDiff};

mod commit_changes;
mod ui;
pub(crate) mod worktree_changes;

fn unified_diffs(
    changes: Vec<TreeChange>,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<UnifiedDiff>> {
    let mut out = Vec::new();
    for diff in changes.into_iter().map(|c| c.unified_diff(repo, 3)) {
        out.push(diff?.context("Can only diff blobs and links, not Commit")?);
    }
    Ok(out)
}
