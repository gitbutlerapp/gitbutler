//! An action to amend an existing commit with selected changes.

use anyhow::{Result, bail};
use but_core::{
    DiffSpec, RefMetadata, RepositoryExt as _,
    commit::{SignCommit, tree_expression::TreeExpression},
    tree::{CreateTreeOutcome, create_tree, create_tree::RejectionReason},
};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, LookupStep as _, Selector, Step, SuccessfulRebase, ToCommitSelector},
};
use gix::prelude::ObjectIdExt as _;

use crate::commit_engine::{Destination, create_commit};

use super::{ChangeSource, cancel_consumed_changes, consumed_changes};

/// What an amend does with a change that doesn't merge into the commit it goes into, as one
/// rewriting lines a later commit wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflicts {
    /// Leave the change out, rejected.
    Reject,
    /// Take it, leaving the commit conflicted.
    Keep,
}

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
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
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
/// When the source worktree has no checkout recorded in the editor, this fails.
/// The amended commit may already have been written to the shared object database
/// at that point; it is unreachable and gets garbage-collected eventually.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
///
/// `conflicts` says what happens to changes that don't merge into `commit`.
pub fn commit_amend<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    source: ChangeSource<'_>,
    conflicts: Conflicts,
) -> Result<CommitAmendOutcome<'ws, 'meta, M>> {
    let AmendInEditorOutcome {
        commit_selector,
        consumed,
        rejected_specs,
    } = amend_in_editor(
        &mut editor,
        commit,
        changes,
        context_lines,
        &source,
        conflicts,
    )?;
    cancel_consumed_changes(&mut editor, &source, consumed, context_lines)?;

    Ok(CommitAmendOutcome {
        rebase: editor.rebase()?,
        commit_selector,
        rejected_specs,
    })
}

/// The result of [amend_in_editor()].
#[derive(Debug)]
pub struct AmendInEditorOutcome {
    /// Selector pointing to the amended commit, if any change was accepted.
    pub commit_selector: Option<Selector>,
    /// The changes that went into the amended commit.
    pub consumed: Vec<DiffSpec>,
    /// Rejected diff specs from commit creation. See [create_commit] for
    /// more details.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
}

/// Like [commit_amend()], but amends `commit` in `editor` without rebasing, and leaves it to the
/// caller to pass [AmendInEditorOutcome::consumed] to [super::cancel_consumed_changes].
pub fn amend_in_editor<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    source: &ChangeSource<'_>,
    conflicts: Conflicts,
) -> Result<AmendInEditorOutcome> {
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
    let repo = source.repo(editor);
    let (new_commit, rejected_specs) = match conflicts {
        Conflicts::Reject => {
            let destination = Destination::AmendCommit {
                commit_id: target_id,
                new_message: None,
            };
            let created = create_commit(repo, destination, changes, context_lines)?;
            (created.new_commit, created.rejected_specs)
        }
        Conflicts::Keep => amend_keeping_conflicts(repo, target_id, changes, context_lines)?,
    };

    let Some(new_commit_id) = new_commit else {
        return Ok(AmendInEditorOutcome {
            commit_selector: None,
            consumed: Vec::new(),
            rejected_specs,
        });
    };
    editor.amend_pick(target_selector, new_commit_id)?;

    Ok(AmendInEditorOutcome {
        commit_selector: Some(target_selector),
        consumed: consumed_changes(all_changes, &rejected_specs),
        rejected_specs,
    })
}

type Rejected = Vec<(RejectionReason, DiffSpec)>;

/// Amend `target_id` in `repo` with `changes` as they are on its checkout's HEAD: what they
/// change there merges into the commit, which stays conflicted where it doesn't merge.
fn amend_keeping_conflicts(
    repo: &gix::Repository,
    target_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> Result<(Option<gix::ObjectId>, Rejected)> {
    let head_tree = repo.head_tree_id_or_empty()?.detach();
    let CreateTreeOutcome {
        rejected_specs,
        destination_tree,
        ..
    } = create_tree(repo, head_tree, changes, context_lines)?;
    let Some(changed_tree) = destination_tree else {
        return Ok((None, rejected_specs));
    };
    let target = but_core::Commit::from_id(target_id.attach(repo))?;
    let mut amended = TreeExpression::try_from(&target)?;
    amended.subtract_add(&head_tree.into(), &changed_tree.into());
    let merged = (amended.merge(repo, repo.merge_options_force_ours()?)?).into_merged_tree(repo)?;
    let mut commit = target.detach();
    merged.apply_to(repo, &mut commit)?;
    let change_id = commit.change_id();
    let id = but_rebase::commit::create(
        repo,
        commit.inner,
        DateMode::CommitterUpdateAuthorKeep,
        SignCommit::IfSignCommitsEnabled,
        Some(change_id),
    )?;
    Ok((Some(id), rejected_specs))
}
