//! Explicit maintenance of virtual-branch metadata after fetching.

use anyhow::Context as _;
use but_core::ref_metadata::ProjectMeta;
use but_db::{ConnectionMut, VbStackHead};

/// Remove unapplied stacks that contain no commits relative to the target, or whose head is missing.
pub fn garbage_collect(
    repo: &gix::Repository,
    project_meta: &ProjectMeta,
    db: &mut ConnectionMut<'_, '_>,
) -> anyhow::Result<()> {
    let Some(target_sha) = project_meta.target_commit_id else {
        return Ok(());
    };
    let mut snapshot = db.virtual_branches().get_snapshot()?.unwrap_or_default();
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut to_remove = Vec::new();
    for stack in snapshot.stacks.iter().filter(|stack| !stack.in_workspace) {
        let head = snapshot
            .heads
            .iter()
            .rev()
            .find(|head| head.stack_id == stack.id);
        if let Ok(stack_head) = stack_head_oid(repo, head, target_sha)
            && (repo.find_commit(stack_head).is_err()
                || stack_head
                    == repo
                        .merge_base_with_graph(stack_head, target_sha, &mut graph)?
                        .detach())
        {
            to_remove.push(stack.id.clone());
        }
    }
    if !to_remove.is_empty() {
        snapshot
            .stacks
            .retain(|stack| !to_remove.contains(&stack.id));
        snapshot
            .heads
            .retain(|head| !to_remove.contains(&head.stack_id));
        db.meta_mut()?.replace_snapshot(&snapshot)?;
    }
    Ok(())
}

fn stack_head_oid(
    repo: &gix::Repository,
    head: Option<&VbStackHead>,
    default_target: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let Some(head) = head else {
        return Ok(default_target);
    };
    let full_name = gix::refs::Category::LocalBranch.to_full_name(head.name.as_str())?;
    let Some(reference) = repo.try_find_reference(full_name.as_ref())? else {
        return head.head_sha.parse().context("Invalid stored stack head");
    };
    reference
        .try_id()
        .map(|id| id.detach())
        .context("Stack head reference did not point to an object id")
}
