use std::ops::Deref;

use anyhow::{Context as _, Result};
use but_oxidize::git2_to_gix_object_id;
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
    /// Cherry-pick, but understands GitButler conflicted states.
    /// Note that it will automatically resolve conflicts in *our* favor, so any tree produced
    /// here can be used.
    ///
    /// This method *should* always be used in favour of native functions.
    fn cherry_pick_gitbutler<'repo>(
        &'repo self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
    ) -> Result<gix::merge::tree::Outcome<'repo>>;

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
        let tree = commit.tree()?;
        if commit.is_conflicted() {
            let conflicted_side = tree
                .get_name(&side)
                .context("Failed to get conflicted side of commit")?;
            self.find_tree(conflicted_side.id())
                .context("failed to find subtree")
        } else {
            self.find_tree(tree.id()).context("failed to find subtree")
        }
    }
}

impl GixRepositoryExt for gix::Repository {
    fn cherry_pick_gitbutler<'repo>(
        &'repo self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
    ) -> Result<gix::merge::tree::Outcome<'repo>> {
        // we need to do a manual 3-way patch merge
        // find the base, which is the parent of to_rebase
        let base = if to_rebase.is_conflicted() {
            // Use to_rebase's recorded base
            self.find_real_tree(
                &git2_to_gix_object_id(to_rebase.id()),
                ConflictedTreeKey::Base,
            )?
        } else {
            let base_commit = to_rebase.parent(0)?;
            // Use the parent's auto-resolution
            self.find_real_tree(&git2_to_gix_object_id(base_commit.id()), Default::default())?
        };
        // Get the auto-resolution
        let ours = self.find_real_tree(&git2_to_gix_object_id(head.id()), Default::default())?;
        // Get the original theirs
        let theirs = self.find_real_tree(
            &git2_to_gix_object_id(to_rebase.id()),
            ConflictedTreeKey::Theirs,
        )?;

        use but_core::RepositoryExt;
        self.merge_trees(
            base,
            ours,
            theirs,
            self.default_merge_labels(),
            self.merge_options_force_ours()?,
        )
        .context("failed to merge trees for cherry pick")
    }

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
