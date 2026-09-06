//! Discard commits from the graph.

use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{Editor, RebasedEditor};

/// Discard one or more commits in a single rebase operation.
///
/// Each commit is removed from history and its parents are reconnected to its
/// children. All removals share a single editor session so only one rebase
/// is performed. Duplicate commit IDs are silently deduplicated.
pub fn discard_commits<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    subject_commits: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<RebasedEditor<'meta, M>> {
    let mut seen = gix::hashtable::HashSet::default();
    let mut count = 0usize;
    for commit_id in subject_commits {
        if !seen.insert(commit_id) {
            continue;
        }
        count += 1;
        let entry = editor.select_commit(commit_id)?;
        let _commit = editor.commit_of(entry)?;
        // A linked worktree checked out on a discarded commit steps down to its parent.
        if let Some((parent, _)) = editor.direct_parents(entry)?.first().copied() {
            for reference in super::linked_worktree_refs_on(&editor, entry)? {
                editor.insert_parent(reference, parent, 0)?;
            }
        }
        editor.remove_commit(entry)?;
    }

    if count == 0 {
        bail!("no commit IDs provided for discard");
    }

    editor.rebase()
}
