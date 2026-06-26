//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, SuccessfulRebase, ToCommitSelector, ToSelector,
    mutate::{InsertSide, RelativeTo, SegmentDelimiter, SelectorSet},
};

use crate::graph_manipulation::determine_parent_selector;

/// Move a commit.
///
/// `editor` is assumed to be aligned with the graph being mutated.
///
/// `subject_commit` - The commit to be moved.
///
/// `anchor` - A git graph node selector to move the subject commit relative to.
///
/// `side` - The side relative to the anchor at which to insert the subject commit.
///
/// The subject commit will be detached from the source segment, and inserted relative
/// to a given anchor (branch or commit).
pub fn move_commit<'ws, 'meta, M: RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    subject_commit: impl ToCommitSelector,
    anchor: impl ToSelector,
    side: InsertSide,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    let editor = move_commit_no_rebase(editor, subject_commit, anchor, side)?;
    editor.rebase()
}

/// Move multiple commits.
///
/// The commits are ordered by parentage before moving so callers do not need to
/// provide them in graph order.
pub fn move_commits<'ws, 'meta, M: RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    subject_commit_ids: impl IntoIterator<Item = gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    let subject_commit_ids = subject_commit_ids.into_iter().collect::<Vec<_>>();
    if subject_commit_ids.is_empty() {
        bail!("No commits were provided to move")
    }

    let ordered_selectors = editor.order_commit_selectors_by_parentage(subject_commit_ids)?;
    let mut ordered_ids = ordered_selectors
        .iter()
        .map(|selector| editor.lookup_pick(*selector))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let ordered_ids = if matches!(side, InsertSide::Above) {
        ordered_ids.reverse();
        ordered_ids
    } else {
        ordered_ids
    };

    let mut subjects = ordered_ids.into_iter();
    let first_subject = subjects
        .next()
        .expect("non-empty commit list always has a first subject");

    let mut editor = move_commit_no_rebase(editor, first_subject, relative_to.clone(), side)?;

    for subject_id in subjects {
        editor = move_commit_no_rebase(editor, subject_id, relative_to.clone(), side)?;
    }

    editor.rebase()
}

/// Move a commit without rebasing.
///
/// `editor` is assumed to be aligned with the graph being mutated.
///
/// `subject_commit` - The commit to be moved.
///
/// `anchor` - A git graph node selector to move the subject commit relative to.
///
/// `side` - The side relative to the anchor at which to insert the subject commit.
///
/// The subject commit will be detached from the source segment, and inserted relative
/// to a given anchor (branch or commit).
///
/// This function mutates the editor graph but does not execute a rebase.
pub fn move_commit_no_rebase<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    subject_commit: impl ToCommitSelector,
    anchor: impl ToSelector,
    side: InsertSide,
) -> anyhow::Result<Editor<'ws, 'meta, M>> {
    let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;

    let commit_delimiter = SegmentDelimiter {
        child: subject_commit_selector,
        parent: subject_commit_selector,
    };

    // Step 1: Determine the parents to disconnect.
    let parent_to_disconnect = determine_parent_selector(&editor, subject_commit_selector)?;

    // Step 2: Disconnect
    editor.disconnect_segment_from(
        commit_delimiter.clone(),
        SelectorSet::All,
        parent_to_disconnect,
        false,
    )?;

    // Step 3: Insert
    editor.insert_segment(anchor, commit_delimiter, side)?;
    Ok(editor)
}
