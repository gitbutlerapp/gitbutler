//! An action to move changes between commits

use anyhow::{Result, bail};
use but_core::{DiffSpec, RepositoryExt, commit::write::DateMode};
use but_graph::{
    MutableNodeGraph, NodeIndex, Rebased,
    edit::{Pick, ToCommitSelector},
};

use crate::tree_manipulation::{ChangesSource, create_tree_without_diff};

/// The result of a move_changes_between_commits operation.
#[derive(Debug)]
pub struct MoveChangesOutcome {
    /// The successful rebase result
    pub rebase: Rebased,
    /// Node index pointing to the source commit (with changes removed)
    pub source_selector: NodeIndex,
    /// Node index pointing to the destination commit (with changes added)
    pub destination_selector: NodeIndex,
}

/// Move changes from one commit to another.
///
/// This operation removes the specified changes from the source commit and
/// applies them to the destination commit using a three-way merge.
///
/// ## Parameters
///
/// - `graph`: The mutable graph to use
/// - `source_commit_id`: The commit to remove changes from
/// - `destination_commit_id`: The commit to add changes to
/// - `changes_to_move`: The changes to move (as "subtraction" specs)
/// - `context_lines`: Number of context lines for hunk matching
///
/// ## Returns
///
/// Returns the rebase outcome along with node indexes pointing to both the
/// modified source and destination commits. The caller should call
/// `outcome.rebase.materialize_changes()` to persist the changes.
pub fn move_changes_between_commits(
    mut graph: MutableNodeGraph,
    source_commit: impl ToCommitSelector,
    destination_commit: impl ToCommitSelector,
    changes_to_move: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<MoveChangesOutcome> {
    let (source_selector, source_commit) = graph.find_selectable_commit(source_commit)?;
    let (destination_selector, destination_commit) =
        graph.find_selectable_commit(destination_commit)?;

    // Early return if source and destination are the same
    if source_commit.id == destination_commit.id {
        // Select the commit to get a valid selector, then just rebase (no-op)
        let outcome = graph.rebase()?;
        return Ok(MoveChangesOutcome {
            rebase: outcome,
            source_selector,
            destination_selector,
        });
    }

    // Step 1: Get the source commit and its tree
    let source_tree_id = {
        let source_commit = source_commit.clone().attach(graph.repo());
        if source_commit.is_conflicted() {
            bail!("Source commit must not be conflicted")
        }
        source_commit.tree
    };

    let (source_tree_without_changes_id, dropped_diffs) = create_tree_without_diff(
        graph.repo(),
        ChangesSource::Commit {
            id: source_commit.id,
        },
        changes_to_move,
        context_lines,
    )?;

    if !dropped_diffs.is_empty() {
        bail!("Failed to extract described changes from source commit");
    }

    let new_source_commit_id = {
        let mut new_source_commit = source_commit.clone();
        new_source_commit.tree = source_tree_without_changes_id;
        graph.new_commit(new_source_commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    graph.replace_commit(source_selector, Pick::new_pick(new_source_commit_id))?;

    // Rebase and get potentially rebased destination commit
    let mut graph = graph.rebase()?.into_mut();
    let (_, rebased_destination_commit) = graph.find_selectable_commit(destination_selector)?;
    let destination_tree_id = {
        let rebased_destination_commit = rebased_destination_commit.clone().attach(graph.repo());
        if rebased_destination_commit.is_conflicted() {
            bail!("Destination commit must not be conflicted")
        }
        rebased_destination_commit.tree
    };

    let destination_tree_with_changes = {
        let repo = graph.repo();
        let (fail_fast_options, conflict_kind) = repo.merge_options_fail_fast()?;
        let mut merge_result = repo.merge_trees(
            source_tree_without_changes_id,
            source_tree_id,
            destination_tree_id,
            Default::default(),
            fail_fast_options,
        )?;

        if merge_result.has_unresolved_conflicts(conflict_kind) {
            bail!("Failed to apply changes to destination commit - merge conflict");
        }

        merge_result.tree.write()?.detach()
    };

    let new_destination_commit_id = {
        let mut commit = rebased_destination_commit;
        commit.tree = destination_tree_with_changes;
        graph.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    graph.replace_commit(
        destination_selector,
        Pick::new_pick(new_destination_commit_id),
    )?;

    let outcome = graph.rebase()?;

    Ok(MoveChangesOutcome {
        rebase: outcome,
        source_selector,
        destination_selector,
    })
}
