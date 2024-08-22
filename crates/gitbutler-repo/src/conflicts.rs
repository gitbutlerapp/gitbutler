//  tree_writer.insert(".conflict-side-0", side0.id(), 0o040000)?;
//  tree_writer.insert(".conflict-side-1", side1.id(), 0o040000)?;
//  tree_writer.insert(".conflict-base-0", base_tree.id(), 0o040000)?;
//  tree_writer.insert(".auto-resolution", resolved_tree_id, 0o040000)?;
//  tree_writer.insert(".conflict-files", conflicted_files_blob, 0o100644)?;

use std::ops::Deref;

pub enum ConflictedTreeKey {
    /// The commit we're rebasing onto "head"
    Ours,
    /// The commit we're rebasing "to rebase"
    Theirs,
    /// The parent of "to rebase"
    Base,
    /// An automatic resolution of conflicts
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
