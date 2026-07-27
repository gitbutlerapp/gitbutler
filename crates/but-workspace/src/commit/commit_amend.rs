//! An action to amend an existing commit with selected changes.

use anyhow::{Result, bail};
use but_core::{DiffSpec, RefMetadata};
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Selector, Step, SuccessfulRebase, ToCommitSelector,
};

use crate::commit_engine::{Destination, create_commit};

use super::{ChangeSource, cancel_consumed_changes};

/// The result of amending a commit in the graph rebase editor.
#[derive(Debug)]
pub struct CommitAmendOutcome<'ws, 'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations. This will be
    /// always provided regardless of whether a commit was actually
    /// created.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
    /// Selector pointing to the amended commit, if the amend was
    /// successful.
    ///
    /// A commit may not be amended if all the diff_specs are rejected. See
    /// [`create_commit`] for more details.
    pub commit_selector: Option<Selector>,
    /// Rejected diff specs from commit creation. See [`create_commit`] for
    /// more details.
    pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
}

/// Amend a commit specified by `commit` selector.
///
/// Similar to other `editor` based functions, this consumes an editor and
/// gives it back as a [`SuccessfulRebase`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes should be committed, and `source` which
/// checkout they are read from - see [`create_commit`] for more details.
///
/// With a [`ChangeSource::Worktree`] source, `commit` may still live anywhere in
/// the editor graph - on a workspace stack or on the branch of any (other)
/// worktree seeded into it. The merge-base override that cancels the consumed
/// changes is keyed to the source checkout, so this must be the first operation
/// on a fresh editor: that checkout must still describe the pre-amend state.
///
/// When the source worktree has no checkout recorded in the editor, this fails
/// without mutating the editor graph. Note that the amended commit may already
/// have been written to the shared object database at that point; it is
/// unreachable and gets garbage-collected eventually.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
pub fn commit_amend<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    source: ChangeSource<'_>,
) -> Result<CommitAmendOutcome<'ws, 'meta, M>> {
    let (target_selector, target) = editor.find_selectable_commit(commit)?;

    let target_id = target.id;
    if target.attach(editor.repo()).is_conflicted() {
        bail!("Cannot amend a conflicted commit")
    }
    // An immutable pick would be replaced in the step graph while the rebase copies
    // its descendants verbatim and never moves the (immutable) refs pointing at it -
    // the amended commit would be written but stay unreachable, with this function
    // still reporting success. Fail fast instead.
    let Step::Pick(target_pick) = editor.lookup_step(target_selector)? else {
        bail!("BUG: Expected pick step from commit selector. This should never happen");
    };
    if !target_pick.mutable {
        bail!(
            "cannot amend into {target_id}: the commit is immutable (not part of a mutable branch)"
        );
    }

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let create_out = create_commit(
        source.repo(&editor),
        Destination::AmendCommit {
            commit_id: target_id,
            new_message: None,
        },
        changes,
        context_lines,
    )?;

    let Some(new_commit_id) = create_out.new_commit else {
        return Ok(CommitAmendOutcome {
            rebase: editor.rebase()?,
            commit_selector: None,
            rejected_specs: create_out.rejected_specs,
        });
    };

    // Runs before `replace` so an unknown worktree fails with zero graph mutation.
    cancel_consumed_changes(
        &mut editor,
        &source,
        all_changes,
        &create_out.rejected_specs,
        context_lines,
    )?;

    editor.replace(target_selector, Step::new_pick(new_commit_id))?;

    Ok(CommitAmendOutcome {
        rebase: editor.rebase()?,
        commit_selector: Some(target_selector),
        rejected_specs: create_out.rejected_specs,
    })
}
