use std::ops::Deref;

use anyhow::{Context as _, Result};
use gitbutler_commit::commit_ext::CommitExt;

#[derive(Default)]
pub enum ConflictedTreeKey {
    /// The commit we're rebasing onto "head"
    Ours,
    /// The commit we're rebasing "to rebase"
    Theirs,
    /// The parent of "to rebase"
    Base,
    /// An automatic resolution of conflicts
    #[default]
    AutoResolution,
    /// A list of conflicted files
    ConflictFiles,
}

impl Deref for ConflictedTreeKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            ConflictedTreeKey::Ours => ".conflict-side-0",
            ConflictedTreeKey::Theirs => ".conflict-side-1",
            ConflictedTreeKey::Base => ".conflict-base-0",
            ConflictedTreeKey::AutoResolution => ".auto-resolution",
            ConflictedTreeKey::ConflictFiles => ".conflict-files",
        }
    }
}

pub trait GixRepositoryExt {
    /// Find the real tree of a commit, which is the tree of the commit if it's not in a conflicted state
    /// or the tree according to `side` if it is conflicted.
    ///
    /// Unless you want to find a particular side, you likely want to pass Default::default()
    /// as the [`side`](ConflictedTreeKey) which will give the automatically resolved resolution
    fn find_real_tree<'repo>(
        &'repo self,
        commit: &'repo gix::Commit,
        side: ConflictedTreeKey,
    ) -> Result<gix::Id<'repo>>;
}

impl GixRepositoryExt for gix::Repository {
    fn find_real_tree<'repo>(
        &'repo self,
        commit: &'repo gix::Commit,
        side: ConflictedTreeKey,
    ) -> Result<gix::Id<'repo>> {
        Ok(if commit.is_conflicted() {
            let tree = commit.tree()?;
            let conflicted_side = tree
                .find_entry(&*side)
                .context("Failed to get conflicted side of commit")?;
            conflicted_side.id()
        } else {
            commit.tree_id()?
        })
    }
}
