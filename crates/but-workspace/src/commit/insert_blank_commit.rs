//! Insertion of a blank commit

use anyhow::Result;
use but_core::RefMetadata;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        CommitIndex, CommitSpec, Editor, RebasedEditor, anchor::Anchor, mutate::InsertSide,
    },
};

/// Inserts a blank commit relative to either a reference or a commit
pub fn insert_blank_commit<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    relative_to: Anchor,
    side: InsertSide,
) -> Result<(RebasedEditor<'meta, M>, CommitIndex)> {
    let commit = editor.empty_commit()?;
    let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;

    let blank_commit_handle =
        editor.insert_commit(relative_to, CommitSpec::untracked(new_id), side)?;

    let outcome = editor.rebase()?;

    Ok((outcome, blank_commit_handle))
}
