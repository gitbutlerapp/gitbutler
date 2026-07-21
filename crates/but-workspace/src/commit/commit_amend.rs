//! An action to amend an existing commit with selected changes.

use anyhow::{Result, bail};
use but_core::DiffSpec;
use but_graph::{
    MutableNodeGraph, NodeIndex, Rebased,
    edit::{Pick, ToCommitSelector},
};

use crate::commit_engine::{Destination, create_commit};

use super::compute_merge_base_override;

/// The result of amending a commit in the mutable graph.
#[derive(Debug)]
pub struct CommitAmendOutcome {
    /// A successful rebase result for continuing operations. This will be
    /// always provided regardless of whether a commit was actually
    /// created.
    pub rebase: Rebased,
    /// Node index pointing to the amended commit, if the amend was
    /// successful.
    ///
    /// A commit may not be amended if all the diff_specs are rejected. See
    /// [`create_commit`] for more details.
    pub commit_selector: Option<NodeIndex>,
    /// Rejected diff specs from commit creation. See [`create_commit`] for
    /// more details.
    pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
}

/// Amend a commit specified by `commit` selector.
///
/// Similar to other graph based functions, this consumes a mutable graph and
/// gives it back as a [`Rebased`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes from the worktree should be committed.
/// See [`create_commit`] for more details.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
pub fn commit_amend(
    mut graph: MutableNodeGraph,
    commit: impl ToCommitSelector,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> Result<CommitAmendOutcome> {
    let (target_selector, target) = graph.find_selectable_commit(commit)?;

    let target_id = target.id;
    if target.attach(graph.repo()).is_conflicted() {
        bail!("Cannot amend a conflicted commit")
    }

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let create_out = create_commit(
        graph.repo(),
        Destination::AmendCommit {
            commit_id: target_id,
            new_message: None,
        },
        changes,
        context_lines,
    )?;

    let Some(new_commit_id) = create_out.new_commit else {
        return Ok(CommitAmendOutcome {
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

    graph.replace_commit(target_selector, Pick::new_pick(new_commit_id))?;

    Ok(CommitAmendOutcome {
        rebase: graph.rebase()?,
        commit_selector: Some(target_selector),
        rejected_specs: create_out.rejected_specs,
    })
}
