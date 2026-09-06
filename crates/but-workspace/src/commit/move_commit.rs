//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    CommitIndex, Editor, RebasedEditor,
    anchor::{Anchor, Connect, Cut, Range},
    mutate::{InsertSide, Reconnect},
};

/// Move multiple commits.
///
/// The commits are ordered by parentage before moving so callers do not need to
/// provide them in graph order: parents before children, and for unrelated commits
/// the entrypoint's history before commits only an auxiliary region reaches.
pub fn move_commits<'meta, M: RefMetadata>(
    editor: Editor<'meta, M>,
    subject_commit_ids: impl IntoIterator<Item = gix::ObjectId>,
    relative_to: Anchor,
    side: InsertSide,
) -> anyhow::Result<RebasedEditor<'meta, M>> {
    let subject_commit_ids = subject_commit_ids.into_iter().collect::<Vec<_>>();
    if subject_commit_ids.is_empty() {
        bail!("No commits were provided to move")
    }

    let subjects = editor.select_commits(subject_commit_ids)?;
    let mut ordered = editor.order_by_parentage(subjects)?;
    if matches!(side, InsertSide::Above) {
        ordered.reverse();
    }

    let mut subjects = ordered.into_iter();
    let first_subject = subjects
        .next()
        .expect("non-empty commit list always has a first subject");

    let mut editor = move_one(editor, first_subject, relative_to.clone(), side)?;

    for subject in subjects {
        editor = move_one(editor, subject, relative_to.clone(), side)?;
    }

    editor.rebase()
}

/// Move one subject: detach it from where it sits — dependents healing past it — and
/// insert it relative to `anchor`. Mutates the graph without rebasing, so the plural
/// driver above can queue several moves into one rebase.
fn move_one<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    subject_commit: CommitIndex,
    anchor: Anchor,
    side: InsertSide,
) -> anyhow::Result<Editor<'meta, M>> {
    let commit_range = Range::single(subject_commit);

    editor.move_range(
        commit_range,
        Cut::All,
        anchor,
        side,
        Connect::Splice,
        Reconnect::Heal,
    )?;
    Ok(editor)
}
