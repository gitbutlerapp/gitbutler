//! An action to split changes off a commit into a new one.

use anyhow::Result;
use but_core::{DiffSpec, RefMetadata};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, Pick, ToCommitSelector, ToSelector, mutate::InsertSide},
};

use crate::commit::{MoveChangesOutcome, move_changes_between_commits};

/// Split `changes` off `source` into a new commit placed on `side` of `relative_to`.
///
/// The outcome's destination is the new commit.
pub fn split_commit<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    source: impl ToCommitSelector,
    relative_to: impl ToSelector,
    side: InsertSide,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<MoveChangesOutcome<'ws, 'meta, M>> {
    let (source, _) = editor.find_selectable_commit(source)?;
    let commit = editor.empty_commit()?;
    let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;
    let split_off =
        editor.split_off(source, Pick::new_untracked_pick(new_id), relative_to, side)?;

    let editor = editor.rebase()?.into_editor();
    move_changes_between_commits(editor, source, split_off, changes, context_lines)
}
