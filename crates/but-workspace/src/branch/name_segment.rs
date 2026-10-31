use but_core::RefMetadata;
use but_graph::WorktreeKind;
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Selector, Step, SuccessfulRebase};

use crate::branch::{
    create_reference,
    create_reference::{Anchor, Position, function::ensure_name_is_available},
};

/// Name the anonymous segment whose top commit is `top` as `ref_name`, as seen from `checkout`.
///
/// If `checkout` is detached at `top`, the new reference points at `top` and `checkout` is
/// attached to it. Otherwise the reference is created right above `top` like
/// [`create_reference()`] would.
///
/// Fail if `ref_name` is already taken.
pub fn name_segment<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    checkout: &WorktreeKind,
    top: gix::ObjectId,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    ensure_name_is_available(&editor, ref_name)?;
    let Some(head) =
        checkout_head(&editor, checkout).filter(|head| is_pick_of(&editor, *head, top))
    else {
        return create_reference(editor, ref_name, Anchor::at_id(top, Position::Above), None);
    };
    let reference = editor.add_step(Step::new_reference(ref_name.to_owned()))?;
    editor.add_edge(reference, head, 0)?;
    match checkout {
        WorktreeKind::Main => editor.set_head_checkout(reference)?,
        WorktreeKind::LinkedId(name) => editor.set_worktree_checkout(name.as_ref(), reference)?,
    }
    editor.rebase()
}

fn checkout_head<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    checkout: &WorktreeKind,
) -> Option<Selector> {
    match checkout {
        WorktreeKind::Main => editor.head(),
        WorktreeKind::LinkedId(name) => editor.worktree_head(name.as_ref()),
    }
}

fn is_pick_of<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    head: Selector,
    top: gix::ObjectId,
) -> bool {
    matches!(editor.lookup_step(head), Ok(Step::Pick(Pick { id, .. })) if id == top)
}
