//! Discard commits from the graph.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, Step, SuccessfulRebase,
    mutate::{SegmentDelimiter, SelectorSet},
};

/// Discard one or more commits in a single rebase operation.
///
/// Each commit is removed from history and its parents are reconnected to its
/// children. All removals share a single editor session so only one rebase
/// is performed. Duplicate commit IDs are silently deduplicated.
pub fn discard_commits<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    subject_commits: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    let mut seen = gix::hashtable::HashSet::default();
    let mut count = 0usize;
    for commit_id in subject_commits {
        if !seen.insert(commit_id) {
            continue;
        }
        count += 1;
        let (selector, _commit) = editor.find_selectable_commit(commit_id)?;

        let delimiter = SegmentDelimiter {
            child: selector,
            parent: selector,
        };

        editor.disconnect_segment_from(delimiter, SelectorSet::All, SelectorSet::All, false)?;
        editor.replace(selector, Step::None)?;
    }

    if count == 0 {
        bail!("no commit IDs provided for discard");
    }

    editor.rebase()
}
