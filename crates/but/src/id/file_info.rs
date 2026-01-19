use std::collections::{BTreeMap, HashMap};

use bstr::BString;

/// Information about committed files needed for CLI ID generation.
pub(crate) struct FileInfo {
    // TODO: It was observed in bd5151cf9 (fix(but status --files): Resolves an
    // issue where the ids shown for committed files are incorrect, 2025-12-29)
    // that sometimes, more than one TreeChange is reported for a (commit,
    // filename) pair even though it's not supposed to happen. (This is why
    // there's a Vec in the definition of `changes` below.) Make sure that this
    // does not happen (possibly by tightening the types involved).
    /// Tree changes indexed by commit ID then filename.
    pub(crate) changes: HashMap<gix::ObjectId, BTreeMap<BString, Vec<but_core::TreeChange>>>,
}

impl FileInfo {
    /// Extracts file information from workspace commits and worktree status.
    ///
    /// This function processes workspace commits to find all changed files in each commit.
    pub(crate) fn from_workspace_commits_and_status<'a, F>(
        workspace_commit_and_first_parent_ids: impl Iterator<
            Item = (&'a gix::ObjectId, &'a Option<gix::ObjectId>),
        >,
        mut changes_fn: F,
    ) -> anyhow::Result<Self>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
    {
        let mut changes: HashMap<gix::ObjectId, BTreeMap<BString, Vec<but_core::TreeChange>>> =
            HashMap::new();
        for (commit_id, parent_id) in workspace_commit_and_first_parent_ids {
            let paths_to_changes = changes.entry(*commit_id).or_default();

            for change in changes_fn(*commit_id, *parent_id)? {
                paths_to_changes
                    .entry(change.path.clone())
                    .or_default()
                    .push(change);
            }
        }
        Ok(Self { changes })
    }
}
