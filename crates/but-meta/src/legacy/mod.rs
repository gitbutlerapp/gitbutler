use std::collections::HashSet;

use anyhow::Context as _;
use but_core::ref_metadata::ProjectMeta;

pub mod storage;

/// Remove unapplied stacks with no commits beyond the target, or with missing heads.
pub fn garbage_collect(
    repo: &gix::Repository,
    project_meta: &ProjectMeta,
    db: &mut but_db::DbHandle,
) -> anyhow::Result<()> {
    let Some(target) = project_meta.target_commit_id else {
        return Ok(());
    };
    let mut tx = db.transaction()?;
    let Some(mut snapshot) = tx.virtual_branches().get_snapshot()? else {
        return Ok(());
    };
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut to_remove = HashSet::new();
    for stack in snapshot.stacks.iter().filter(|stack| !stack.in_workspace) {
        let head = snapshot
            .heads
            .iter()
            .filter(|head| head.stack_id == stack.id)
            .max_by_key(|head| head.position);
        if let Ok(head) = stack_head_oid(repo, head, target)
            && (repo.find_commit(head).is_err()
                || head
                    == repo
                        .merge_base_with_graph(head, target, &mut graph)?
                        .context("Could not find a merge-base between stack head and target")?
                        .detach())
        {
            to_remove.insert(stack.id.clone());
        }
    }
    if to_remove.is_empty() {
        return Ok(());
    }
    snapshot
        .stacks
        .retain(|stack| !to_remove.contains(&stack.id));
    snapshot
        .heads
        .retain(|head| !to_remove.contains(&head.stack_id));
    tx.virtual_branches_mut()?.replace_snapshot(&snapshot)?;
    tx.commit()?;
    Ok(())
}

fn stack_head_oid(
    repo: &gix::Repository,
    head: Option<&but_db::VbStackHead>,
    target: gix::ObjectId,
) -> anyhow::Result<gix::ObjectId> {
    let Some(head) = head else {
        return Ok(target);
    };
    let full_name = gix::refs::FullName::try_from(format!("refs/heads/{}", head.name))
        .context("Stack head name could not be turned into a full branch reference name")?;
    let Some(reference) = repo.try_find_reference(full_name.as_ref())? else {
        return Ok(gix::ObjectId::from_hex(head.head_sha.as_bytes())?);
    };
    reference
        .try_id()
        .map(|id| id.detach())
        .context("Stack head reference did not point to an object id")
}
