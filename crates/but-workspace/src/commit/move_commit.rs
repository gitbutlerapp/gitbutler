//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, Step, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};

/// Move multiple commits.
///
/// The commits are ordered by parentage before moving so callers do not need to
/// provide them in graph order.
///
/// Each subject is plucked from its old slot - replaced in place by [`Step::None`] -
/// and inserted relative to `relative_to`. Leaving a placeholder behind means the source
/// topology is never rewritten, so every reference anchored in it keeps its position and
/// resolves through the placeholder to the commit below, which is exactly what a branch
/// a commit was moved out of should do.
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

    let mut ordered_selectors = editor.order_commit_selectors_by_parentage(subject_commit_ids)?;
    // Every insert lands adjacent to the anchor, so consecutive inserts stack away from
    // it. Parentage order already reads base-first, which is what inserting below wants;
    // inserting above walks the other way.
    if matches!(side, InsertSide::Above) {
        ordered_selectors.reverse();
    }

    let mut editor = editor;
    let mut plucked = Vec::with_capacity(ordered_selectors.len());
    for selector in ordered_selectors {
        // The first-parent edge is the slot the commit sat in and stays with the
        // placeholder; a merge commit's remaining parents are part of the commit itself
        // and travel with it.
        let mut parents = editor.direct_parents(selector)?;
        parents.sort_by_key(|(_, order)| *order);
        let merge_parents = parents.split_off(1.min(parents.len()));
        let step = editor.replace(selector, Step::None)?;
        plucked.push((step, selector, merge_parents));
    }
    for (step, placeholder, merge_parents) in plucked {
        let inserted = editor.insert(relative_to.clone(), step, side)?;
        for (parent, order) in merge_parents {
            editor.remove_edges(placeholder, parent)?;
            editor.add_edge(inserted, parent, order)?;
        }
    }

    editor.rebase()
}
