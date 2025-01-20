use anyhow::{Context, Result};
use gix::bstr::{BString, ByteVec};
use tracing::instrument;

/// An extension trait that should avoid pulling in large amounts of dependency so it can be used
/// in more places without causing cycles.
/// `gitbutler_repo::RepositoryExt` may not be usable everywhere due to that.
pub trait RepositoryExtLite {
    /// Exclude files that are larger than `limit_in_bytes` (eg. database.sql which may never be intended to be committed)
    /// so they don't show up in the next diff.
    /// If `0` this method will have no effect.
    fn ignore_large_files_in_diffs(&self, limit_in_bytes: u64) -> Result<()>;
}

impl RepositoryExtLite for git2::Repository {
    #[instrument(level = tracing::Level::DEBUG, skip(self), err(Debug))]
    fn ignore_large_files_in_diffs(&self, limit_in_bytes: u64) -> Result<()> {
        if limit_in_bytes == 0 {
            return Ok(());
        }
        use gix::bstr::ByteSlice;
        let repo = gix::open(self.path())?;
        let worktree_dir = repo
            .work_dir()
            .context("All repos are expected to have a worktree")?;
        let files_to_exclude: Vec<_> = repo
            .dirwalk_iter(
                repo.index_or_empty()?,
                None::<BString>,
                Default::default(),
                repo.dirwalk_options()?
                    .emit_ignored(None)
                    .emit_pruned(false)
                    .emit_untracked(gix::dir::walk::EmissionMode::Matching),
            )?
            .filter_map(Result::ok)
            .filter_map(|item| {
                let path = worktree_dir.join(gix::path::from_bstr(item.entry.rela_path.as_bstr()));
                let file_is_too_large = path
                    .metadata()
                    .is_ok_and(|md| md.is_file() && md.len() > limit_in_bytes);
                file_is_too_large
                    .then(|| Vec::from(item.entry.rela_path).into_string().ok())
                    .flatten()
            })
            .collect();
        // TODO(ST): refactor this to be path-safe and ' ' save - the returned list is space separated (!!)
        //           Just make sure this isn't needed anymore.
        let ignore_list = files_to_exclude.join(" ");
        // In-memory, libgit2 internal ignore rule
        self.add_ignore_rule(&ignore_list)?;
        Ok(())
    }
}
