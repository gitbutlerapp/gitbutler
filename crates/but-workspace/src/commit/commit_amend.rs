//! An action to amend an existing commit with selected changes.

use anyhow::{Result, bail};
use but_core::{DiffSpec, RefMetadata};
use but_rebase::graph_rebase::{Editor, Selector, Step, SuccessfulRebase, ToCommitSelector};
use gix::bstr::BStr;

use crate::commit_engine::{Destination, create_commit};

use super::compute_merge_base_override;

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
    /// The requested changes that actually made it into the amended commit,
    /// i.e. all requested `changes` minus [`Self::rejected_specs`].
    ///
    /// Empty when no commit was created.
    pub consumed_specs: Vec<DiffSpec>,
}

/// Where the amended changes come from, and hence which checkout should receive
/// the merge-base override that cancels them out after materialization.
enum ChangeSource<'a> {
    /// The main worktree of the repository the editor was created for.
    Head,
    /// The linked worktree with this name, read through this from-disk repository.
    Worktree {
        repo: &'a gix::Repository,
        name: &'a BStr,
    },
}

/// Amend a commit specified by `commit` selector.
///
/// Similar to other `editor` based functions, this consumes an editor and
/// gives it back as a [`SuccessfulRebase`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes from the worktree should be committed.
/// See [`create_commit`] for more details.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
pub fn commit_amend<'ws, 'meta, M: RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> Result<CommitAmendOutcome<'ws, 'meta, M>> {
    commit_amend_inner(editor, commit, changes, context_lines, ChangeSource::Head)
}

/// Like [`commit_amend()`], but `changes` are uncommitted changes of the linked
/// worktree named `worktree_name`, read from its from-disk repository
/// `worktree_repo`, while `commit` may live anywhere in the editor graph - on a
/// workspace stack or on the branch of any (other) worktree seeded into it.
///
/// `worktree_repo` must share the editor repo's object database (i.e. be a plain
/// from-disk open of the linked worktree, without object memory): new objects are
/// written loose to disk, which makes them immediately visible to the editor's
/// in-memory repository.
///
/// The merge-base override that cancels the consumed changes is keyed to the
/// worktree's own checkout, so this must be the first operation on a fresh
/// editor - the worktree checkout selector must still describe the pre-amend
/// state of `worktree_repo`'s `HEAD`.
///
/// When `worktree_name` has no checkout recorded in the editor (unknown,
/// archived, detached `HEAD`, or worktree tips not seeded into the graph), this
/// fails without mutating the editor graph. Note that the amended commit may
/// already have been written to the shared object database at that point; it is
/// unreachable and gets garbage-collected eventually.
pub fn commit_amend_from_worktree<'ws, 'meta, M: RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    worktree_repo: &gix::Repository,
    worktree_name: &BStr,
) -> Result<CommitAmendOutcome<'ws, 'meta, M>> {
    commit_amend_inner(
        editor,
        commit,
        changes,
        context_lines,
        ChangeSource::Worktree {
            repo: worktree_repo,
            name: worktree_name,
        },
    )
}

fn commit_amend_inner<'ws, 'meta, M: RefMetadata>(
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

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let source_repo = match &source {
        ChangeSource::Head => editor.repo(),
        ChangeSource::Worktree { repo, .. } => *repo,
    };
    let create_out = create_commit(
        source_repo,
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
            consumed_specs: Vec::new(),
        });
    };

    // Tell the editor which changes were consumed so the checkout's snapshot
    // merge doesn't reintroduce them as uncommitted changes.
    let rejected_paths: std::collections::BTreeSet<_> = create_out
        .rejected_specs
        .iter()
        .map(|(_, spec)| &spec.path)
        .collect();
    let consumed: Vec<_> = all_changes
        .into_iter()
        .filter(|spec| !rejected_paths.contains(&spec.path))
        .collect();
    if !consumed.is_empty() {
        let merge_base = compute_merge_base_override(source_repo, consumed.clone(), context_lines)?;
        match &source {
            ChangeSource::Head => editor.set_merge_base_override(merge_base),
            // Runs before `replace` so an unknown worktree fails with zero
            // graph mutation.
            ChangeSource::Worktree { name, .. } => {
                editor.set_worktree_merge_base_override(name, merge_base)?
            }
        }
    }

    editor.replace(target_selector, Step::new_pick(new_commit_id))?;

    Ok(CommitAmendOutcome {
        rebase: editor.rebase()?,
        commit_selector: Some(target_selector),
        rejected_specs: create_out.rejected_specs,
        consumed_specs: consumed,
    })
}
