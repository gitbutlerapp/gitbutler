//! Bring the editor's `HEAD` into a workspace with a workspace commit.

use anyhow::Context as _;
use but_core::{RefMetadata, WORKSPACE_REF_NAME};
use but_error::bail_precondition;
use but_rebase::graph_rebase::{
    Editor, Pick, Selector, Step, SuccessfulRebase, mutate::InsertSide,
};

use crate::{WorkspaceCommit, commit::Stack};

/// Return the workspace commit, creating one if none is found.
pub fn ensure_workspace_commit<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    workspace_reference: Selector,
) -> anyhow::Result<(SuccessfulRebase<'ws, 'meta, M>, Selector)> {
    if let Some(workspace_commit) = editor.workspace_commit(workspace_reference) {
        return Ok((editor.rebase()?, workspace_commit));
    }

    let (_, below) = editor.find_reference_target(workspace_reference)?;
    let id = write_workspace_commit(editor.repo(), &below)?;
    let workspace_commit = editor.insert(
        workspace_reference,
        Step::Pick(Pick::new_workspace_pick(id)),
        InsertSide::Below,
    )?;

    Ok((editor.rebase()?, workspace_commit))
}

/// Return a [SuccessfulRebase] that contains a newly created workspace commit with the current
/// `HEAD` as its parent and the workspace reference pointing to said workspace commit, together
/// with a selector pointing to the workspace commit.
///
/// Fail if `HEAD` is already on the workspace reference.
pub fn switch_to_workspace<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
) -> anyhow::Result<(SuccessfulRebase<'ws, 'meta, M>, Selector)> {
    if editor.workspace_reference().is_some() {
        bail_precondition!("HEAD is already on the workspace reference");
    }
    let head = editor
        .head()
        .context("HEAD has no checkout recorded in the editor")?;
    let workspace_ref = gix::refs::FullName::try_from(WORKSPACE_REF_NAME)?;
    if let Some(existing) = editor.try_select_reference(workspace_ref.as_ref()) {
        editor.replace_with_none(existing)?;
    }
    let (_, below) = editor.find_commit_target(head)?;
    let commit = write_workspace_commit(editor.repo(), &below)?;
    let workspace_commit = editor.add_step(Step::Pick(Pick::new_workspace_pick(commit)))?;
    editor.add_edge(workspace_commit, head, 0)?;
    let workspace_reference = editor.add_step(Step::new_reference(workspace_ref))?;
    editor.add_edge(workspace_reference, workspace_commit, 0)?;
    editor.set_head_checkout(workspace_reference)?;
    Ok((editor.rebase()?, workspace_commit))
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
