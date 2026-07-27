//! Move a commit within or across branches and stacks.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Selector, Step, SuccessfulRebase, ToCommitSelector, ToSelector,
    mutate::{InsertSide, RelativeTo, SegmentDelimiter, SelectorSet},
};

use crate::graph_manipulation::determine_parent_selector;

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

    // When inserting below a reference, resolve it and its target commit up front so
    // co-located refs can be kept pinned to that commit rather than being dragged
    // onto the moved commits. A reference is a node in the parent chain, so closing
    // the gap left by a detached subject must reconnect to the commit the reference
    // names, not to the reference node itself.
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
    // Detach every subject against the original topology before inserting any of
    // them, so a later subject's parent isn't decided against a graph an earlier
    // insertion already mutated.
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

/// Detach `subject_commit` from its source segment and close the gap, returning its
/// selector for a later insertion. Does not insert.
///
/// When `destination_reference_target` is set and the subject's parent is exactly
/// that destination reference node, the subject's children are reconnected to the
/// commit the reference names rather than to the reference node - so a co-located
/// reference stays pinned to its commit instead of riding along with the move.
fn disconnect_commit<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    subject_commit: impl ToCommitSelector,
    destination_reference_target: Option<Selector>,
) -> anyhow::Result<Selector> {
    let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;

    let commit_delimiter = SegmentDelimiter {
        child: subject_commit_selector,
        parent: subject_commit_selector,
    };

    // Step 1: Determine the parents to disconnect.
    let parent_to_disconnect = determine_parent_selector(editor, subject_commit_selector)?;
    // The gap-closing fix only applies when the subject hangs off exactly the
    // destination reference node; anything else keeps today's plain reconnect.
    let sole_parent = match &parent_to_disconnect {
        SelectorSet::Some(parents) => match parents.as_slice() {
            [only] => Some(only.to_selector(editor)?),
            _ => None,
        },
        SelectorSet::All | SelectorSet::None => None,
    };
    let reference_parent = match (destination_reference_target, sole_parent) {
        (Some(destination_target), Some(reference))
            if matches!(editor.lookup_step(reference)?, Step::Reference { .. }) =>
        {
            let (reference_target, _) = editor.find_reference_target(reference)?;
            (reference_target == destination_target).then_some((reference, reference_target))
        }
        _ => None,
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

/// Move the destination reference's own reference-children onto its target commit,
/// so inserting below the destination reference lifts only that reference onto the
/// moved commits - sibling refs co-located at the same commit stay put.
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

/// Re-point `child`'s edge from the reference node onto the commit it names, keeping
/// its ordering intact. A no-op if `child` has no edge to `reference`.
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
