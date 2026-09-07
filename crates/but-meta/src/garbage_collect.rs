//! Explicit maintenance of workspace metadata after fetching.

use anyhow::Context as _;
use but_core::ref_metadata::{ProjectMeta, WorkspaceStack};
use but_db::ConnectionMut;

/// Remove unapplied stacks that contain no commits relative to the target, or whose head is missing.
/// Workspace membership changes use the caller's connection and leave independent branch metadata intact.
pub fn garbage_collect(
    repo: &gix::Repository,
    project_meta: &ProjectMeta,
    db: &mut ConnectionMut<'_, '_>,
) -> anyhow::Result<()> {
    let Some(target_sha) = project_meta.target_commit_id else {
        return Ok(());
    };
    let metadata = db.meta()?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    for (name, workspace) in metadata.workspaces() {
        let mut to_remove = Vec::new();
        for stack in workspace
            .stacks
            .iter()
            .filter(|stack| !stack.is_in_workspace())
        {
            let should_remove = match stack_head_oid(repo, stack) {
                Ok(None) => true,
                Ok(Some(head)) => {
                    repo.find_commit(head).is_err()
                        || head
                            == repo
                                .merge_base_with_graph(head, target_sha, &mut graph)?
                                .detach()
                }
                Err(_) => false,
            };
            if should_remove {
                to_remove.push(stack.id);
            }
        }
        if !to_remove.is_empty() {
            let mut workspace = workspace.clone();
            workspace
                .stacks
                .retain(|stack| !to_remove.contains(&stack.id));
            db.meta_mut()?.set_workspace(name, &workspace)?;
        }
    }
    Ok(())
}

fn stack_head_oid(
    repo: &gix::Repository,
    stack: &WorkspaceStack,
) -> anyhow::Result<Option<gix::ObjectId>> {
    let Some(name) = stack.name() else {
        return Ok(None);
    };
    let Some(reference) = repo.try_find_reference(name)? else {
        return Ok(None);
    };
    reference
        .try_id()
        .map(|id| Some(id.detach()))
        .context("Stack head reference did not point to an object id")
}
