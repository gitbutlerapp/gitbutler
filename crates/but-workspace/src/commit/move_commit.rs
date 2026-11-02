//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_rebase::graph_rebase::{
    Editor, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};

/// The commits are ordered by parentage before moving so callers do not need to
/// provide them in graph order.
///
/// Each subject hands its children to its first parent, so a branch a commit was moved out
/// of rests on the commit below, and is then placed relative to `relative_to`.
pub fn move_commits<'ws, 'meta>(
    editor: Editor<'ws, 'meta>,
    subject_commit_ids: impl IntoIterator<Item = gix::ObjectId>,
    relative_to: RelativeTo,
    side: InsertSide,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta>> {
    let subject_commit_ids = subject_commit_ids.into_iter().collect::<Vec<_>>();
    if subject_commit_ids.is_empty() {
        bail!("No commits were provided to move")
    }

    let mut ordered_selectors = editor.order_commit_selectors_by_parentage(subject_commit_ids)?;
    // Every insert lands adjacent to the anchor, so consecutive inserts stack away from
    // it. Parentage order already reads base-first, which is what inserting below wants;
    // inserting above walks the other way.
    if matches!(side, InsertSide::Above) {
        ordered_selectors.reverse();
    }

    let mut editor = editor;
    for selector in ordered_selectors {
        editor.move_pick(selector, relative_to.clone(), side)?;
    }

    editor.rebase()
}
