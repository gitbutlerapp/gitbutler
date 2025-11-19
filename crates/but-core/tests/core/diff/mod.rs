use anyhow::Context as _;
use bstr::{BString, ByteVec};
use but_core::{TreeChange, UnifiedPatch};

mod commit_changes;
mod ui;
pub(crate) mod worktree_changes;

fn unified_patches(
    changes: &[TreeChange],
    repo: &gix::Repository,
) -> anyhow::Result<Vec<UnifiedPatch>> {
    let mut out = Vec::new();
    for patch in changes.iter().map(|c| c.unified_patch(repo, 3)) {
        out.push(patch?.context("Can only diff blobs and links, not Commit")?);
    }
    Ok(out)
}

fn unified_diffs(changes: &[TreeChange], repo: &gix::Repository) -> anyhow::Result<BString> {
    let mut out = BString::default();
    for diff in changes.iter().map(|c| c.unified_diff(repo, 3)) {
        out.push_str(diff?.context("Can only diff blobs and links, not Commit")?);
        out.push(b'\n');
    }
    Ok(out)
}
