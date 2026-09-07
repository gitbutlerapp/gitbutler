//! Explicit maintenance of virtual-branch metadata after fetching.

use anyhow::Context as _;
use but_core::ref_metadata::ProjectMeta;
use but_db::ConnectionMut;

use crate::{
    legacy_storage::{legacy_to_snapshot, snapshot_to_legacy},
    virtual_branches_legacy_types::Stack,
};

/// Remove unapplied stacks that contain no commits relative to the target, or whose head is missing.
pub fn garbage_collect(
    repo: &gix::Repository,
    project_meta: &ProjectMeta,
    db: &mut ConnectionMut<'_, '_>,
) -> anyhow::Result<()> {
    let Some(target_sha) = project_meta.target_commit_id else {
        return Ok(());
    };
    let snapshot = db.virtual_branches().get_snapshot()?.unwrap_or_default();
    let mut data = snapshot_to_legacy(&snapshot)?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut to_remove = Vec::new();
    for stack in data.branches.values().filter(|stack| !stack.in_workspace) {
        if let Ok(stack_head) = stack_head_oid(repo, stack, target_sha)
            && (repo.find_commit(stack_head).is_err()
                || stack_head
                    == repo
                        .merge_base_with_graph(stack_head, target_sha, &mut graph)?
                        .detach())
        {
            to_remove.push(stack.id);
        }
    }
    if !to_remove.is_empty() {
        for stack_id in to_remove {
            data.branches.remove(&stack_id);
        }
        db.meta_mut()?
            .replace_snapshot(&legacy_to_snapshot(&data)?)?;
    }
    Ok(())
}

fn stack_head_oid(
    repo: &gix::Repository,
    stack: &Stack,
    default_target: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let Some(head) = stack.heads.last() else {
        return Ok(default_target);
    };
    let full_name = gix::refs::Category::LocalBranch.to_full_name(head.name.as_str())?;
    let Some(reference) = repo.try_find_reference(full_name.as_ref())? else {
        return Ok(head.head);
    };
    reference
        .try_id()
        .map(|id| id.detach())
        .context("Stack head reference did not point to an object id")
}
