//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Selector, Step, SuccessfulRebase, ToCommitSelector, ToSelector,
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

    if matches!(side, InsertSide::Above) {
        ordered_ids.reverse();
    }

    let destination_reference = if matches!(side, InsertSide::Below) {
        match &relative_to {
            RelativeTo::Reference(name) => {
                let reference = editor.select_reference(name.as_ref())?;
                Some((reference, editor.find_reference_target(reference)?.0))
            }
            RelativeTo::Commit(_) => None,
        }
    } else {
        None
    };
    let mut editor = editor;
    let mut detached = Vec::with_capacity(ordered_ids.len());
    for subject in ordered_ids {
        detached.push(disconnect_commit(
            &mut editor,
            subject,
            destination_reference.map(|(_, target)| target),
        )?);
    }
    if let Some((reference, target)) = destination_reference {
        detach_reference_children(&mut editor, reference, target)?;
    }
    for subject in detached {
        editor.insert_segment(
            relative_to.clone(),
            SegmentDelimiter {
                child: subject,
                parent: subject,
            },
            side,
        )?;
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
    let subject_commit_selector = disconnect_commit(&mut editor, subject_commit, None)?;

    let commit_delimiter = SegmentDelimiter {
        child: subject_commit_selector,
        parent: subject_commit_selector,
    };

    editor.insert_segment(anchor, commit_delimiter, side)?;
    Ok(editor)
}

fn disconnect_commit<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    subject_commit: impl ToCommitSelector,
    destination_reference_target: Option<Selector>,
) -> anyhow::Result<but_rebase::graph_rebase::Selector> {
    let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;

    let commit_delimiter = SegmentDelimiter {
        child: subject_commit_selector,
        parent: subject_commit_selector,
    };

    // Step 1: Determine the parents to disconnect.
    let parent_to_disconnect = determine_parent_selector(editor, subject_commit_selector)?;
    let reference_parent = match (destination_reference_target, &parent_to_disconnect) {
        (Some(destination_target), SelectorSet::Some(parents)) if parents.as_slice().len() == 1 => {
            let reference = parents.as_slice()[0].to_selector(editor)?;
            if matches!(editor.lookup_step(reference)?, Step::Reference { .. }) {
                let (reference_target, _) = editor.find_reference_target(reference)?;
                (reference_target == destination_target).then_some((reference, reference_target))
            } else {
                None
            }
        }
        (None, _) | (Some(_), SelectorSet::All | SelectorSet::None | SelectorSet::Some(_)) => None,
    };
    let children = reference_parent
        .is_some()
        .then(|| editor.direct_children(subject_commit_selector))
        .transpose()?
        .unwrap_or_default();

    // Step 2: Disconnect
    editor.disconnect_segment_from(
        commit_delimiter.clone(),
        SelectorSet::All,
        parent_to_disconnect,
        false,
    )?;
    if let Some((reference, target)) = reference_parent {
        for (child, desired_order) in children {
            reconnect_child_to_reference_target(editor, child, reference, target, desired_order)?;
        }
    }
    Ok(subject_commit_selector)
}

fn detach_reference_children<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    reference: Selector,
    target: Selector,
) -> anyhow::Result<()> {
    for (child, desired_order) in editor.direct_children(reference)? {
        if matches!(editor.lookup_step(child)?, Step::Reference { .. }) {
            reconnect_child_to_reference_target(editor, child, reference, target, desired_order)?;
        }
    }
    Ok(())
}

fn reconnect_child_to_reference_target<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    child: Selector,
    reference: Selector,
    target: Selector,
    desired_order: usize,
) -> anyhow::Result<()> {
    if editor.remove_edges(child, reference)?.is_empty() {
        return Ok(());
    }
    let used_orders = editor
        .direct_parents(child)?
        .into_iter()
        .map(|(_, order)| order)
        .collect::<std::collections::HashSet<_>>();
    let order = (desired_order..)
        .find(|order| !used_orders.contains(order))
        .expect("a free edge order always exists");
    editor.add_edge(child, target, order)?;
    Ok(())
}
