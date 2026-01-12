use std::ops::Deref;

use anyhow::{Context as _, Result};
use but_oxidize::{ObjectIdExt as _, OidExt, RepoExt};
use gitbutler_commit::commit_ext::CommitExt;

mod repository_ext;
pub use repository_ext::RepositoryExtLite;

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

pub trait RepositoryExt {
    /// Find the real tree of a commit, which is the tree of the commit if it's not in a conflicted state
    /// or the tree according to `side` if it is conflicted.
    ///
    /// Unless you want to find a particular side, you likely want to pass Default::default()
    /// as the [`side`](ConflictedTreeKey) which will give the automatically resolved resolution
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: ConflictedTreeKey,
    ) -> Result<git2::Tree<'_>>;
}

pub trait GixRepositoryExt {
    /// Find the real tree of a commit, which is the tree of the commit if it's not in a conflicted state
    /// or the tree according to `side` if it is conflicted.
    ///
    /// Unless you want to find a particular side, you likely want to pass Default::default()
    /// as the [`side`](ConflictedTreeKey) which will give the automatically resolved resolution
    fn find_real_tree<'repo>(
        &'repo self,
        commit_id: &gix::oid,
        side: ConflictedTreeKey,
    ) -> Result<gix::Id<'repo>>;
}

impl RepositoryExt for git2::Repository {
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: ConflictedTreeKey,
    ) -> Result<git2::Tree<'_>> {
        let gix_repo = self.to_gix_repo()?;
        let tree_id = gix_repo
            .find_real_tree(&commit.id().to_gix(), side)?
            .to_git2();
        Ok(self.find_tree(tree_id)?)
    }
}

impl GixRepositoryExt for gix::Repository {
    fn find_real_tree<'repo>(
        &'repo self,
        commit_id: &gix::oid,
        side: ConflictedTreeKey,
    ) -> Result<gix::Id<'repo>> {
        let commit = self.find_commit(commit_id)?;
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
