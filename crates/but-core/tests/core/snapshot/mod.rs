mod index_create_and_resolve;
mod worktree_create_and_resolve;

mod utils {
    use but_core::snapshot;

    /// Produce all args needed for creating a snapshot tree, and assure everything is selected.
    pub fn args_for_worktree_changes(
        repo: &gix::Repository,
    ) -> anyhow::Result<(gix::Id<'_>, snapshot::create_tree::State)> {
        let changes = but_core::diff::worktree_changes_no_renames(repo)?;
        let state = snapshot::create_tree::State {
            selection: changes
                .changes
                .iter()
                .map(|c| c.path.clone())
                .chain(changes.ignored_changes.iter().map(|c| c.path.clone()))
                .collect(),
            changes,
            head: false,
        };
        let head_tree_id = repo.head_tree_id_or_empty()?;

        Ok((head_tree_id, state))
    }
}
pub use utils::args_for_worktree_changes;
