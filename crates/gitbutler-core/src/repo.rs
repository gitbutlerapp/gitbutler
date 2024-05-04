use anyhow::Result;

/// The GbRepository trait provides methods which are building blocks for Gitbutler functionality.
pub trait GbRepository {
    /// Returns the number of uncommitted lines of code (added plus removed) in the repository. Included untracked files.
    fn changed_lines_count(&self) -> Result<usize>;
}

impl GbRepository for git2::Repository {
    fn changed_lines_count(&self) -> Result<usize> {
        let head_tree = self.head()?.peel_to_commit()?.tree()?;
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true);
        let diff = self.diff_tree_to_workdir_with_index(Some(&head_tree), Some(&mut opts));
        let stats = diff?.stats()?;
        Ok(stats.deletions() + stats.insertions())
    }
}
