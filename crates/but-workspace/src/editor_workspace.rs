//! Bring the editor's `HEAD` into a workspace with a workspace commit, and out of it again.

use anyhow::Context as _;
use but_core::WORKSPACE_REF_NAME;
use but_error::bail_precondition;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Pick, Selector, Step, SuccessfulRebase, mutate::InsertSide,
};
use gix::{bstr::ByteSlice as _, refs::Category};
use nonempty::NonEmpty;

use crate::{WorkspaceCommit, commit::Stack};

/// Return the workspace commit, creating one if none is found.
fn ensure_workspace_commit<'ws, 'meta>(
    mut editor: Editor<'ws, 'meta>,
    workspace_reference: Selector,
) -> anyhow::Result<(SuccessfulRebase<'ws, 'meta>, Selector)> {
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
pub fn switch_to_workspace<'ws, 'meta>(
    mut editor: Editor<'ws, 'meta>,
) -> anyhow::Result<(SuccessfulRebase<'ws, 'meta>, Selector)> {
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

/// Return the workspace commit `HEAD` is on, first switching to a workspace if it isn't on one
/// ([switch_to_workspace]), or giving the workspace reference a workspace commit if it has
/// none.
pub fn enter_workspace<'ws, 'meta>(
    editor: Editor<'ws, 'meta>,
) -> anyhow::Result<(SuccessfulRebase<'ws, 'meta>, Selector)> {
    match editor.workspace_reference() {
        Some(workspace_reference) => ensure_workspace_commit(editor, workspace_reference),
        None => switch_to_workspace(editor),
    }
}

/// Enter the workspace for a new stack, including any stacks saved when leaving it.
pub(crate) fn enter_workspace_for_new_stack<'ws, 'meta>(
    editor: Editor<'ws, 'meta>,
    restore_saved_stacks: bool,
) -> anyhow::Result<(Editor<'ws, 'meta>, Selector)> {
    if !restore_saved_stacks
        || editor.workspace_reference().is_some()
        || editor
            .repo()
            .try_find_reference(WORKSPACE_REF_NAME)?
            .is_none()
    {
        let (rebase, commit) = enter_workspace(editor)?;
        return Ok((rebase.into_editor(), commit));
    }

    let rebase = editor.rebase()?;
    let stacks = previously_applied_stacks(rebase.repo(), rebase.db())?;
    let rebase =
        switch_to_workspace_with_stacks(rebase.into_editor(), &stacks)?.map_err(|(_, err)| {
            if err
                .downcast_ref::<but_rebase::merge::ConflictErrorContext>()
                .is_some()
            {
                anyhow::anyhow!(
                    "Cannot restore workspace because previously applied branches conflict"
                )
            } else {
                err
            }
        })?;
    let editor = rebase.into_editor();
    let commit = editor
        .workspace_reference()
        .and_then(|reference| editor.workspace_commit(reference))
        .context("restoring workspace stacks must create a workspace commit")?;
    Ok((editor, commit))
}

/// The surviving, nonarchived branches of saved applied stacks, tip first.
pub fn previously_applied_stacks(
    repo: &gix::Repository,
    db: &but_db::DbHandle,
) -> anyhow::Result<Vec<NonEmpty<gix::refs::FullName>>> {
    let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let workspace_meta = db.meta().workspace(workspace_ref.as_ref())?;
    let mut existing_stacks = Vec::new();
    for stack in workspace_meta.stacks(but_core::ref_metadata::StackKind::Applied) {
        // A deleted tip must not hide surviving lower branches in the saved stack.
        // Only absence is recoverable; propagate reference lookup errors.
        let mut existing_branches = Vec::new();
        for branch in stack.branches.iter().filter(|branch| !branch.archived) {
            if repo.try_find_reference(branch.ref_name.as_ref())?.is_some() {
                existing_branches.push(branch.ref_name.clone());
            }
        }
        existing_stacks.extend(NonEmpty::from_vec(existing_branches));
    }
    Ok(existing_stacks)
}

/// Put a workspace commit on top of `HEAD` and check it out, then apply `stacks`, each given
/// tip first, to it.
///
/// If a stack can't be applied, its index in `stacks` is returned with the error, which carries a
/// [`but_rebase::merge::ConflictErrorContext`] if it conflicts with the stacks before it.
pub fn switch_to_workspace_with_stacks<'ws, 'meta>(
    editor: Editor<'ws, 'meta>,
    stacks: &[NonEmpty<gix::refs::FullName>],
) -> anyhow::Result<Result<SuccessfulRebase<'ws, 'meta>, (usize, anyhow::Error)>> {
    let (mut rebase, _) = switch_to_workspace(editor)?;
    for (order, stack) in stacks.iter().enumerate() {
        rebase = match crate::branch::apply::apply_saved_stack(
            rebase.into_editor(),
            stack.head.as_ref(),
            order,
        ) {
            Ok(outcome) => outcome.rebase,
            Err(err) => return Ok(Err((order, err))),
        };
    }
    let mut editor = rebase.into_editor();
    if let Some(target) = editor.target_selector() {
        let applied_tips = workspace_tips(&editor)?;
        for NonEmpty {
            head: tip,
            tail: lower_branches,
        } in stacks
        {
            if !editor
                .try_select_reference(tip.as_ref())
                .is_some_and(|tip| applied_tips.contains(&tip))
            {
                continue;
            }
            for lower_branch in lower_branches {
                stack_lower_branch(&mut editor, target, tip.as_ref(), lower_branch.as_ref())?;
            }
        }
    }
    Ok(Ok(editor.rebase()?))
}

/// If `lower_branch` points to a commit in the lane of `tip` and is not already in a lane, move it
/// into the lane.
fn stack_lower_branch(
    editor: &mut Editor<'_, '_>,
    target: Selector,
    tip: &gix::refs::FullNameRef,
    lower_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let Some(lower) = editor.try_select_reference(lower_branch) else {
        return Ok(());
    };
    // Must not already be in a lane.
    if !editor.direct_children(lower)?.is_empty() {
        return Ok(());
    }
    let [(commit, _)] = editor.direct_parents(lower)?[..] else {
        return Ok(());
    };
    // Find the other end of the edge that we must break (the child's commit).
    let mut step = editor.select_reference(tip)?;
    let child_of_commit = loop {
        if step == target {
            return Ok(());
        }
        let Some((parent, _)) = editor
            .direct_parents(step)?
            .into_iter()
            .min_by_key(|(_, order)| *order)
        else {
            return Ok(());
        };
        if parent == commit {
            break step;
        }
        step = parent;
    };
    for order in editor.remove_edges(child_of_commit, commit)? {
        editor.add_edge(child_of_commit, lower, order)?;
    }
    Ok(())
}

/// Check out `branch` because that was asked for, see [Editor::switch_head()].
///
/// A workspace that this edit entered isn't kept, as nothing is on it then.
pub fn switch_to_branch<'ws, 'meta>(
    mut editor: Editor<'ws, 'meta>,
    branch: &gix::refs::FullNameRef,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta>> {
    let reference = editor.select_reference(branch)?;
    if editor.head() == Some(reference) {
        return editor.rebase();
    }
    let was_on_workspace =
        (editor.repo().head_name()?).is_some_and(|name| name.as_bstr() == WORKSPACE_REF_NAME);
    let entered = editor.workspace_reference().filter(|_| !was_on_workspace);
    if let Some(workspace_reference) = entered {
        dissolve_workspace(&mut editor, workspace_reference)?;
    }
    editor.switch_head(reference)?;
    editor.rebase()
}

/// What the workspace `HEAD` is on merges, skipping steps removed from a lane and any lane that
/// only reaches the target or its local branch. Empty if `HEAD` isn't on the workspace reference.
pub fn workspace_tips(editor: &Editor<'_, '_>) -> anyhow::Result<Vec<Selector>> {
    let Some(workspace_reference) = editor.workspace_reference() else {
        return Ok(Vec::new());
    };
    let top = editor
        .workspace_commit(workspace_reference)
        .unwrap_or(workspace_reference);
    let mut tips = Vec::new();
    for (lane, _) in editor.direct_parents(top)? {
        tips.extend(lane_tip(editor, lane)?);
    }
    Ok(tips)
}

/// The first step from `lane` down that a workspace merges for its own sake, skipping steps
/// removed from the lane, the target's local branch and GitButler's own references. `None` if
/// the lane only reaches the target.
fn lane_tip(editor: &Editor<'_, '_>, lane: Selector) -> anyhow::Result<Option<Selector>> {
    let target = editor.target_selector();
    let mut step = Some(lane);
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
            return Ok(Some(selector));
        }
        step = editor
            .direct_parents(selector)?
            .first()
            .map(|(parent, _)| *parent);
    }
    Ok(None)
}

/// Take `lane` out of the workspace commit `HEAD` is on if it has no [lane_tip], unless it's
/// the only lane.
pub(crate) fn remove_lane_without_tip(
    editor: &mut Editor<'_, '_>,
    lane: Selector,
) -> anyhow::Result<()> {
    let Some(workspace_commit) = editor
        .workspace_reference()
        .and_then(|workspace_reference| editor.workspace_commit(workspace_reference))
    else {
        return Ok(());
    };
    if editor.direct_parents(workspace_commit)?.len() > 1 && lane_tip(editor, lane)?.is_none() {
        editor.remove_edges(workspace_commit, lane)?;
    }
    Ok(())
}

/// Undo [switch_to_workspace] if `editor` has at most one [workspace_tips] left, moving `HEAD` to
/// that branch, or to the local target if there are none.
///
/// The workspace stays if the tip isn't a branch, or if the destination's tree differs from what
/// the workspace represents: its own tree, or the target's if it has no tips. There is nothing to
/// do if `HEAD` isn't on it.
pub fn leave_unnecessary_workspace<'ws, 'meta>(
    mut editor: Editor<'ws, 'meta>,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta>> {
    let Some((workspace_reference, checkout)) = way_out(&editor)? else {
        return editor.rebase();
    };
    dissolve_workspace(&mut editor, workspace_reference)?;
    editor.set_head_checkout(checkout)?;
    editor.rebase()
}

pub(crate) fn dissolve_workspace(
    editor: &mut Editor<'_, '_>,
    workspace_reference: Selector,
) -> anyhow::Result<()> {
    if let Some(workspace_commit) = editor.workspace_commit(workspace_reference) {
        editor.prune(workspace_commit)?;
    }
    editor.replace_with_none(workspace_reference)?;
    Ok(())
}

/// The workspace reference `HEAD` is on and what [leave_unnecessary_workspace] would check out
/// instead, if the workspace is unnecessary.
fn way_out(editor: &Editor<'_, '_>) -> anyhow::Result<Option<(Selector, Selector)>> {
    let Some(workspace_reference) = editor.workspace_reference() else {
        return Ok(None);
    };
    let (checkout, represented_by) = match workspace_tips(editor)?.as_slice() {
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
        return Ok(None);
    };
    let (_, represented) = editor.find_commit_target(represented_by)?;
    let (_, destination) = editor.find_commit_target(checkout)?;
    Ok((represented.inner.tree == destination.inner.tree)
        .then_some((workspace_reference, checkout)))
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
