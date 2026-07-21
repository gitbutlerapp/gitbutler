//! An action to perform a reword of a commit

use anyhow::Result;
use bstr::BStr;
use but_core::commit::write::DateMode;
use but_graph::{
    MutableNodeGraph, NodeIndex, Rebased,
    edit::{Pick, ToCommitSelector},
};

/// This action will rewrite a commit and any relevant history so it uses
/// the new name.
///
/// Returns a node index to the rewritten commit
pub fn reword(
    mut graph: MutableNodeGraph,
    commit: impl ToCommitSelector,
    new_message: &BStr,
) -> Result<(Rebased, NodeIndex)> {
    let (target_selector, mut commit) = graph.find_selectable_commit(commit)?;

    commit.message = new_message.to_owned();
    let new_id = graph.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?;

    graph.replace_commit(target_selector, Pick::new_pick(new_id))?;

    let outcome = graph.rebase()?;

    Ok((outcome, target_selector))
}
