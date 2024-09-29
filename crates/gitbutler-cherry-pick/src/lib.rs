//  tree_writer.insert(".conflict-side-0", side0.id(), 0o040000)?;
//  tree_writer.insert(".conflict-side-1", side1.id(), 0o040000)?;
//  tree_writer.insert(".conflict-base-0", base_tree.id(), 0o040000)?;
//  tree_writer.insert(".auto-resolution", resolved_tree_id, 0o040000)?;
//  tree_writer.insert(".conflict-files", conflicted_files_blob, 0o100644)?;

use std::ops::Deref;

use anyhow::Context;
use git2::MergeOptions;
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

pub trait RepositoryExt {
    fn cherry_pick_gitbutler(
        &self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
        merge_options: Option<&MergeOptions>,
    ) -> Result<git2::Index, anyhow::Error>;
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: ConflictedTreeKey,
    ) -> Result<git2::Tree, anyhow::Error>;
}

impl RepositoryExt for git2::Repository {
    /// cherry-pick, but understands GitButler conflicted states
    ///
    /// cherry_pick_gitbutler should always be used in favour of libgit2 or gitoxide
    /// cherry pick functions
    fn cherry_pick_gitbutler(
        &self,
        head: &git2::Commit,
        to_rebase: &git2::Commit,
        merge_options: Option<&MergeOptions>,
    ) -> Result<git2::Index, anyhow::Error> {
        // we need to do a manual 3-way patch merge
        // find the base, which is the parent of to_rebase
        let base = dbg!(if to_rebase.is_conflicted() {
            // Use to_rebase's recorded base
            self.find_real_tree(to_rebase, ConflictedTreeKey::Base)?
        } else {
            let base_commit = to_rebase.parent(0)?;
            // Use the parent's auto-resolution
            self.find_real_tree(&base_commit, Default::default())?
        });
        // Get the auto-resolution
        let ours = dbg!(self.find_real_tree(head, Default::default())?);
        // Get the original theirs
        let thiers = dbg!(self.find_real_tree(to_rebase, ConflictedTreeKey::Theirs)?);

        self.merge_trees(&base, &ours, &thiers, merge_options)
            .context("failed to merge trees for cherry pick")
    }

    /// Find the real tree of a commit, which is the tree of the commit if it's not in a conflicted state
    /// or the parent parent tree if it is in a conflicted state
    ///
    /// Unless you want to find a particular side, you likly want to pass Default::default()
    /// as the ConfclitedTreeKey which will give the automatically resolved resolution
    fn find_real_tree(
        &self,
        commit: &git2::Commit,
        side: ConflictedTreeKey,
    ) -> Result<git2::Tree, anyhow::Error> {
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
