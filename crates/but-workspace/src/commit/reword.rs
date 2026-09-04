//! An action to perform a reword of a commit

use anyhow::Result;
use bstr::BStr;
use but_core::RefMetadata;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{CommitIndex, CommitSpec, Editor, RebasedEditor},
};

/// This action will rewrite a commit and any relevant history so it uses
/// the new name.
///
/// Returns a entry to the rewritten commit
pub fn reword<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    commit: CommitIndex,
    new_message: &BStr,
) -> Result<(RebasedEditor<'meta, M>, CommitIndex)> {
    let target_entry = commit;
    let mut commit = editor.commit_of(target_entry)?;

    commit.message = but_core::commit::rewrite_conflict_markers_on_message_change(
        commit.message.as_ref(),
        new_message.to_owned(),
    );
    let new_id = editor.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;

    editor.replace_commit(target_entry, CommitSpec::new(new_id))?;

    let outcome = editor.rebase()?;

    Ok((outcome, target_entry))
}
