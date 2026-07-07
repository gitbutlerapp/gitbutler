//! Discard commits from the graph.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, Step, SuccessfulRebase,
    mutate::{SelectorSet, StepRange},
};

/// Discard one or more commits in a single rebase operation.
///
/// Each commit is removed from history and its parents are reconnected to its
/// children. All removals share a single editor session so only one rebase
/// is performed. Duplicate commit IDs are silently deduplicated.
pub fn discard_commits<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    subject_commits: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<SuccessfulRebase<'meta, M>> {
    let mut seen = gix::hashtable::HashSet::default();
    let mut count = 0usize;
    for commit_id in subject_commits {
        if !seen.insert(commit_id) {
            continue;
        }
        count += 1;
        let (selector, _commit) = editor.find_selectable_commit(commit_id)?;

        let range = StepRange {
            child: selector,
            parent: selector,
        };

        editor.disconnect_range_from(range, SelectorSet::All, SelectorSet::All, false)?;
        editor.replace(selector, Step::None)?;
    }

    if count == 0 {
        bail!("no commit IDs provided for discard");
    }

    editor.rebase()
}
