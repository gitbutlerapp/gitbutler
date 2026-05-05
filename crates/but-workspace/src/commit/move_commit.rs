//! Move a commit within or across branches and stacks.

use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, SuccessfulRebase, ToCommitSelector, ToSelector,
    mutate::{InsertSide, SegmentDelimiter, SelectorSet},
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
