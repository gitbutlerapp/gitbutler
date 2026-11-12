//! Bring the editor's `HEAD` into a workspace with a workspace commit.

use anyhow::Context as _;
use but_core::{RefMetadata, WORKSPACE_REF_NAME};
use but_error::bail_precondition;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Pick, Selector, Step, SuccessfulRebase, mutate::InsertSide,
};
use gix::{bstr::ByteSlice as _, refs::Category};

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

/// What the workspace `HEAD` is on merges, skipping steps removed from a lane and any lane that
/// only reaches the target or its local branch. Empty if `HEAD` isn't on the workspace reference.
pub fn workspace_tips<M: RefMetadata>(editor: &Editor<'_, '_, M>) -> anyhow::Result<Vec<Selector>> {
    let Some(workspace_reference) = editor.workspace_reference() else {
        return Ok(Vec::new());
    };
    let top = editor
        .workspace_commit(workspace_reference)
        .unwrap_or(workspace_reference);
    let target = editor.target_selector();
    let mut tips = Vec::new();
    for (parent, _) in editor.direct_parents(top)? {
        let mut step = Some(parent);
        while let Some(selector) = step.filter(|selector| Some(*selector) != target) {
            let is_passthrough = match editor.lookup_step(selector)? {
                Step::None => true,
                Step::Reference { refname, .. } => {
                    editor.target_ref() == Some(refname.as_ref())
                        || refname.as_bstr().starts_with_str("refs/heads/gitbutler/")
                }
                Step::Pick(_) => false,
            };
            if !is_passthrough {
                tips.push(selector);
                break;
            }
            step = editor
                .direct_parents(selector)?
                .first()
                .map(|(parent, _)| *parent);
        }
    }
    Ok(tips)
}

/// Undo [switch_to_workspace] if `editor` has at most one [workspace_tips] left, moving `HEAD` to
/// that branch, or to the local target if there are none.
///
/// The workspace stays if the tip isn't a branch, or if the destination's tree differs from what
/// the workspace represents: its own tree, or the target's if it has no tips. There is nothing to
/// do if `HEAD` isn't on it.
pub fn leave_unnecessary_workspace<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    let Some(workspace_reference) = editor.workspace_reference() else {
        return editor.rebase();
    };
    let (checkout, represented_by) = match workspace_tips(&editor)?.as_slice() {
        [] => (
            editor
                .target_ref()
                .and_then(|target| editor.try_select_reference(target)),
            editor.target_selector(),
        ),
        [tip]
            if matches!(
                editor.lookup_step(*tip)?,
                Step::Reference { refname, .. }
                    if refname.category() == Some(Category::LocalBranch)
            ) =>
        {
            (Some(*tip), Some(workspace_reference))
        }
        _ => (None, None),
    };
    let (Some(checkout), Some(represented_by)) = (checkout, represented_by) else {
        return editor.rebase();
    };
    let (_, represented) = editor.find_commit_target(represented_by)?;
    let (_, destination) = editor.find_commit_target(checkout)?;
    if represented.inner.tree != destination.inner.tree {
        return editor.rebase();
    }

    if let Some(workspace_commit) = editor.workspace_commit(workspace_reference) {
        editor.prune(workspace_commit)?;
    }
    editor.replace_with_none(workspace_reference)?;
    editor.set_head_checkout(checkout)?;
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
