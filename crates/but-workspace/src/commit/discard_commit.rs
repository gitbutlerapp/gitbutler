//! Discard commits from the graph.

use anyhow::bail;
use but_graph::{
    MutableNodeGraph, Rebased,
    edit::{SegmentDelimiter, SelectorSet},
};

/// Discard one or more commits in a single rebase operation.
///
/// Each commit is removed from history and its parents are reconnected to its
/// children. All removals share a single edit session so only one rebase
/// is performed. Duplicate commit IDs are silently deduplicated.
pub fn discard_commits(
    mut graph: MutableNodeGraph,
    subject_commits: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<Rebased> {
    let mut seen = gix::hashtable::HashSet::default();
    let mut count = 0usize;
    for commit_id in subject_commits {
        if !seen.insert(commit_id) {
            continue;
        }
        count += 1;
        let (selector, _commit) = graph.find_selectable_commit(commit_id)?;

        let delimiter = SegmentDelimiter {
            child: selector,
            parent: selector,
        };

        graph.disconnect_segment_from(delimiter, SelectorSet::All, SelectorSet::All, false)?;
        graph.remove(selector)?;
    }

    if count == 0 {
        bail!("no commit IDs provided for discard");
    }

    graph.rebase()
}
