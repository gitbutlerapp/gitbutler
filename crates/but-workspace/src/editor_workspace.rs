//! Bring the editor's `HEAD` into a workspace with a workspace commit.

use but_core::RefMetadata;
use but_rebase::graph_rebase::{Editor, Pick, Step, SuccessfulRebase, mutate::InsertSide};

use crate::{WorkspaceCommit, commit::Stack};

/// Insert a workspace commit below the workspace reference if `HEAD` is on it without one.
pub fn ensure_workspace_commit<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    if editor.workspace_commit().is_none()
        && let Some(workspace_reference) = editor.workspace_reference()
    {
        let (_, below) = editor.find_reference_target(workspace_reference)?;
        let commit = write_workspace_commit(editor.repo(), &below)?;
        editor.insert(
            workspace_reference,
            Step::Pick(Pick::new_workspace_pick(commit)),
            InsertSide::Below,
        )?;
    }
    editor.rebase()
}

fn write_workspace_commit(
    repo: &gix::Repository,
    below: &but_core::CommitOwned,
) -> anyhow::Result<gix::ObjectId> {
    let mut commit = WorkspaceCommit::new_from_stacks(Vec::<Stack>::new(), repo.object_hash());
    commit.tree = below.inner.tree;
    commit.parents = vec![below.id].into();
    WorkspaceCommit::fixup_times(&mut commit, repo);
    Ok(repo.write_object(&commit)?.detach())
}
