//! An action to create a new commit relative to a commit or reference.

use anyhow::{Context as _, Result};
use but_core::{DiffSpec, RefMetadata};
use but_rebase::graph_rebase::{
    Editor, LookupStep, Pick, Selector, Step, SuccessfulRebase, ToSelector, mutate::InsertSide,
};

use crate::commit_engine::{Destination, create_commit};

use super::{
    ChangeSource, Conflicts, cancel_consumed_changes, commit_amend::amend_keeping_conflicts,
    consumed_changes,
};

/// The result of creating and inserting a new commit in the graph rebase editor.
#[derive(Debug)]
pub struct CommitCreateOutcome<'ws, 'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations. This will be
    /// always provided regardless of whether a commit was actually
    /// created.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
    /// Selector pointing to the newly created commit, if one was created.
    ///
    /// A commit may not be created if all the diff_specs are rejected. See
    /// [`create_commit`] for more details.
    pub commit_selector: Option<Selector>,
    /// Rejected diff specs from commit creation. See [`create_commit`] for
    /// more details.
    pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
}

/// Create a commit from `changes` and insert it relative to `relative_to` on `side`.
///
/// Similar to other `editor` based functions, this consumes an editor and
/// gives it back as a [`SuccessfulRebase`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes should be committed, and `source` which
/// checkout they are read from - see [`create_commit`] for more details, and
/// [`commit_amend`](super::commit_amend()) for what a [`ChangeSource::Worktree`]
/// requires.
///
/// `relative_to` and `side` determine the position to insert the commit.
/// See [`InsertSide`] to learn more about insertion semantics.
///
/// `message` will be the message used for the newly created commit.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
///
/// `conflicts` says what happens to changes that don't merge onto the new commit's parent.
#[expect(clippy::too_many_arguments)]
pub fn commit_create<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    changes: Vec<DiffSpec>,
    relative_to: impl ToSelector,
    side: InsertSide,
    message: &str,
    context_lines: u32,
    source: ChangeSource<'_>,
    conflicts: Conflicts,
) -> Result<CommitCreateOutcome<'ws, 'meta, M>> {
    let relative_to_selector = relative_to.to_selector(&editor)?;
    let parent_commit_id =
        parent_commit_id_for_new_commit(&editor, editor.lookup_step(relative_to_selector)?, side)?;

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let repo = source.repo(&editor);
    let new_commit = |changes| {
        let destination = Destination::NewCommit {
            parent_commit_id,
            stack_segment: None,
            message: message.to_owned(),
        };
        create_commit(repo, destination, changes, context_lines)
    };
    let (new_commit, rejected_specs) = match (conflicts, parent_commit_id) {
        (Conflicts::Keep, Some(_)) => {
            let requested = changes.len();
            let empty = new_commit(Vec::new())?.new_commit;
            let empty = empty.context("BUG: an empty commit is always made")?;
            let (amended, rejected) = amend_keeping_conflicts(repo, empty, changes, context_lines)?;
            let all_rejected = requested > 0 && rejected.len() == requested;
            (amended.filter(|_| !all_rejected), rejected)
        }
        (Conflicts::Reject, _) | (Conflicts::Keep, None) => {
            let created = new_commit(changes)?;
            (created.new_commit, created.rejected_specs)
        }
    };

    let Some(new_commit_id) = new_commit else {
        return Ok(CommitCreateOutcome {
            rebase: editor.rebase()?,
            commit_selector: None,
            rejected_specs,
        });
    };

    // Runs before `insert` so an unknown worktree fails with zero graph mutation.
    cancel_consumed_changes(
        &mut editor,
        &source,
        consumed_changes(all_changes, &rejected_specs),
        context_lines,
    )?;

    let commit_selector = editor.insert(
        relative_to_selector,
        Step::new_untracked_pick(new_commit_id),
        side,
    )?;

    Ok(CommitCreateOutcome {
        rebase: editor.rebase()?,
        commit_selector: Some(commit_selector),
        rejected_specs,
    })
}

fn parent_commit_id_for_new_commit<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    target_step: Step,
    side: InsertSide,
) -> Result<Option<gix::ObjectId>> {
    Ok(match (target_step, side) {
        (Step::Pick(Pick { id, .. }), InsertSide::Above) => Some(id),
        (Step::Pick(Pick { id, .. }), InsertSide::Below) => {
            let commit = editor.find_commit(id)?;
            commit.parents.first().copied()
        }
        (Step::Reference { refname, .. }, _) => Some(editor.find_reference_target(refname)?.1.id),
        (Step::None, _) => None,
    })
}
