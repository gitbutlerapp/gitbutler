//! Insertion of a blank commit

use anyhow::Result;
use but_core::commit::write::DateMode;
use but_graph::{
    MutableNodeGraph, NodeIndex, Rebased,
    edit::{InsertSide, Pick, ToSelector},
};

/// Inserts a blank commit relative to either a reference or a commit
pub fn insert_blank_commit(
    mut graph: MutableNodeGraph,
    side: InsertSide,
    relative_to: impl ToSelector,
) -> Result<(Rebased, NodeIndex)> {
    let commit = graph.empty_commit()?;
    let new_id = graph.new_commit(commit, DateMode::CommitterUpdateAuthorUpdate)?;

    let blank_commit_selector =
        graph.insert_commit_with(relative_to, Pick::new_untracked_pick(new_id), side)?;

    let outcome = graph.rebase()?;

    Ok((outcome, blank_commit_selector))
}
