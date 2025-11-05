mod amend_commit;
mod new_commit;
mod refs_update;

pub mod utils {
    pub fn assure_no_worktree_changes(repo: &gix::Repository) -> anyhow::Result<()> {
        assert_eq!(
            but_core::diff::worktree_changes(repo)?.changes.len(),
            0,
            "all changes are seemingly incorporated"
        );
        Ok(())
    }
}
