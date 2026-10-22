//! Writing a checkout's target tree to the worktree and the index.

mod plan;
mod protect;
mod transaction;

use anyhow::Context as _;

use plan::Plan;

/// Turn the worktree of `repo`, which matched `trees.baseline` (in Git form) when it was
/// snapshotted, into `trees.target`, writing only the entries that differ.
///
/// Nothing changes if checkout refuses or fails. Files are written through the repository's
/// worktree filters, so clean/smudge drivers like git-crypt or Git LFS see the same conversions
/// as with Git, and the filters run before anything in the worktree is removed. Everything
/// checkout removes is checked against the snapshot after the filters ran, and a failure while
/// moving files into place is undone. A crash in the middle leaves the originals in a staging
/// directory below the Git directory.
/// `.git/index` is updated for the written and deleted paths only, keeping all other entries as
/// they are. If `conflicts` is set, its unresolved conflicts are recorded as index stages.
/// Like Git, it refuses before changing anything if a path it would write or delete has staged
/// content that differs from `trees.head`, the tree the checkout leaves, and isn't in the
/// worktree either. Paths the worktree already has as `trees.new_head` has them aren't written,
/// but their index entries follow `trees.new_head` unless the user staged something there.
/// `index` is what the index records before the checkout, and is written as the last step.
pub(super) fn write_to_worktree(
    repo: &gix::Repository,
    trees: Trees,
    conflicts: Conflicts<'_>,
    mut index: LockedIndex,
) -> anyhow::Result<()> {
    let plan = Plan::new(&index.entries, &trees, conflicts);
    protect::staged_content(&index.entries, &trees, &plan)?;
    let staged = transaction::stage(repo, &trees.target, &plan)?;
    let clearances = protect::worktree(repo, &trees.baseline, &plan)?;
    let placed = transaction::commit(repo, staged, &clearances)?;
    plan.edit(&mut index.entries, &placed);
    index.write()
}

/// `.git/index`, locked for the whole checkout, and the entries the checkout works with.
pub(super) struct LockedIndex {
    lock: gix::lock::File,
    /// What `.git/index` records, or what it will record once written.
    pub entries: gix::index::File,
}

impl LockedIndex {
    /// Lock `.git/index` and read it. Like Git, checkout then refuses before touching anything
    /// while another process holds the lock, and nothing else changes the index meanwhile.
    pub fn acquire(repo: &gix::Repository) -> anyhow::Result<Self> {
        let lock = gix::lock::File::acquire_to_update_resource(
            repo.index_path(),
            gix::lock::acquire::Fail::Immediately,
            None,
            0,
        )
        .context("Repository index is locked by another process")?;
        Ok(LockedIndex {
            lock,
            entries: repo.index_or_empty()?.into_owned_or_cloned(),
        })
    }

    /// Write `entries` to `.git/index` and release the lock.
    pub fn write(self) -> anyhow::Result<()> {
        let mut out = std::io::BufWriter::new(self.lock);
        self.entries.write_to(&mut out, Default::default())?;
        out.into_inner().map_err(|err| err.into_error())?.commit()?;
        Ok(())
    }
}

/// The unresolved conflicts of the merge that produced the target, if any.
pub(super) type Conflicts<'a> = Option<&'a gix::merge::tree::Outcome<'a>>;

/// The trees a checkout moves between, as index states.
pub(super) struct Trees {
    /// The tree the checkout leaves.
    pub head: gix::index::State,
    /// The tree the checkout moves to.
    pub new_head: gix::index::State,
    /// The worktree in Git form, as snapshotted before the checkout.
    pub baseline: gix::index::State,
    /// `new_head` with the worktree's changes merged in: what the worktree becomes.
    pub target: gix::index::State,
}

impl Trees {
    pub fn new(
        repo: &gix::Repository,
        head: gix::ObjectId,
        new_head: gix::ObjectId,
        [baseline, target]: [gix::ObjectId; 2],
    ) -> anyhow::Result<Self> {
        repo.workdir().context("checkout needs a worktree")?;
        Ok(Trees {
            head: repo.index_from_tree(&head)?.into(),
            new_head: repo.index_from_tree(&new_head)?.into(),
            baseline: repo.index_from_tree(&baseline)?.into(),
            target: repo.index_from_tree(&target)?.into(),
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use gix::object::tree::EntryKind;

    use super::Trees;

    #[test]
    fn deletions_do_not_follow_a_symlinked_parent_directory() -> anyhow::Result<()> {
        let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&outside)?;
        std::fs::write(outside.join("f"), "outside")?;
        let repo = gix::init(tmp.path().join("repo"))?;

        let blob = repo.write_blob("tracked")?.detach();
        let mut editor = repo.empty_tree().edit()?;
        editor.upsert("dir/f", EntryKind::Blob, blob)?;
        let baseline = editor.write()?.detach();
        let target = repo.empty_tree().id;
        // The worktree had `dir/f` when it was snapshotted, then `dir` was replaced by a symlink.
        std::os::unix::fs::symlink("../outside", repo.workdir().unwrap().join("dir"))?;

        let trees = Trees::new(&repo, baseline, target, [baseline, target])?;
        super::write_to_worktree(&repo, trees, None, super::LockedIndex::acquire(&repo)?)?;

        assert!(
            outside.join("f").is_file(),
            "like Git, a file behind a symlinked directory isn't in the worktree, so it isn't deleted"
        );
        Ok(())
    }
}
