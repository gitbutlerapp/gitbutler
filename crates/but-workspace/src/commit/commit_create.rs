//! An action to create a new commit relative to a commit or reference.

use anyhow::Result;
use but_core::DiffSpec;
use but_graph::{
    MutableNodeGraph, NodeIndex, NodeKind, Rebased,
    edit::{InsertSide, Pick, ToSelector},
};

use crate::commit_engine::{Destination, create_commit};

use super::compute_merge_base_override;

/// The result of creating and inserting a new commit in the mutable graph.
#[derive(Debug)]
pub struct CommitCreateOutcome {
    /// A successful rebase result for continuing operations. This will be
    /// always provided regardless of whether a commit was actually
    /// created.
    pub rebase: Rebased,
    /// Node index pointing to the newly created commit, if one was created.
    ///
    /// A commit may not be created if all the diff_specs are rejected. See
    /// [`create_commit`] for more details.
    pub commit_selector: Option<NodeIndex>,
    /// Rejected diff specs from commit creation. See [`create_commit`] for
    /// more details.
    pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
}

/// Create a commit from `changes` and insert it relative to `relative_to` on `side`.
///
/// Similar to other graph based functions, this consumes a mutable graph and
/// gives it back as a [`Rebased`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes from the worktree should be committed.
/// See [`create_commit`] for more details.
///
/// `relative_to` and `side` determine the position to insert the commit.
/// See [`InsertSide`] to learn more about insertion semantics.
///
/// `message` will be the message used for the newly created commit.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
pub fn commit_create(
    mut graph: MutableNodeGraph,
    changes: Vec<DiffSpec>,
    relative_to: impl ToSelector,
    side: InsertSide,
    message: &str,
    context_lines: u32,
) -> Result<CommitCreateOutcome> {
    let relative_to_selector = relative_to.to_selector(&graph)?;
    let parent_commit_id = parent_commit_id_for_new_commit(&graph, relative_to_selector, side)?;

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let create_out = create_commit(
        graph.repo(),
        Destination::NewCommit {
            parent_commit_id,
            stack_segment: None,
            message: message.to_owned(),
        },
        changes,
        context_lines,
    )?;

    let Some(new_commit_id) = create_out.new_commit else {
        return Ok(CommitCreateOutcome {
            rebase: graph.rebase()?,
            commit_selector: None,
            rejected_specs: create_out.rejected_specs,
        });
    };

    // Tell the editor which changes were consumed so the checkout's snapshot
    // merge doesn't reintroduce them as uncommitted changes.
    let rejected_paths: std::collections::BTreeSet<_> = create_out
        .rejected_specs
        .iter()
        .map(|(_, spec)| &spec.path)
        .collect();
    let consumed: Vec<_> = all_changes
        .into_iter()
        .filter(|spec| !rejected_paths.contains(&spec.path))
        .collect();
    if !consumed.is_empty() {
        let merge_base = compute_merge_base_override(graph.repo(), consumed, context_lines)?;
        graph.set_merge_base_override(merge_base);
    }

    let commit_selector = graph.insert_commit_with(
        relative_to_selector,
        Pick::new_untracked_pick(new_commit_id),
        side,
    )?;

    Ok(CommitCreateOutcome {
        rebase: graph.rebase()?,
        commit_selector: Some(commit_selector),
        rejected_specs: create_out.rejected_specs,
    })
}

fn parent_commit_id_for_new_commit(
    graph: &MutableNodeGraph,
    target: NodeIndex,
    side: InsertSide,
) -> Result<Option<gix::ObjectId>> {
    // `pick_at` also covers convergence boundaries, which are selectable as
    // commits when `relative_to` resolves a commit id onto a
    // convergence-boundary node.
    if let Some(pick) = graph.pick_at(target) {
        return Ok(match side {
            InsertSide::Above => Some(pick.id),
            InsertSide::Below => {
                let commit = graph.find_commit(pick.id)?;
                commit.parents.first().copied()
            }
        });
    }
    Ok(match graph.nodes()[target].kind() {
        NodeKind::Reference(_) => Some(graph.find_reference_target(target)?.1.id),
        // Tombstones and shallow boundaries read as removed nodes: the new
        // commit gets no parent. Commits and convergence boundaries were
        // already handled by `pick_at` above.
        _ => None,
    })
}
