//! The ability to create a Git representation of diverse 'state' that can be restored at a later time.

///
pub mod create_tree {
    use bstr::BString;

    /// A way to determine what should be included in the snapshot when calling [create_tree()](function::create_tree).
    pub struct State<'a> {
        /// The result of a previous worktree changes call.
        ///
        /// It contains detailed information about the complete set of possible changes to become part of the worktree.
        pub changes: &'a but_core::WorktreeChanges,
        /// Repository-relative and slash-separated paths that match any change in the  [`changes`](State::changes) field.
        /// **It's an error if there is no match.** as there is not supposed to be a snapshot without a change to the working tree.
        pub selection: Vec<BString>,
        /// If `true`, store the current `HEAD` reference, i.e. its target, as well as the targets of all refs it's pointing to by symbolic link.
        pub head: bool,
    }

    /// Contains all state that the snapshot contains.
    #[derive(Debug, Copy, Clone)]
    pub struct Outcome {
        /// The snapshot itself, with all the subtrees available that are also listed in this structure.
        pub snapshot_tree: gix::ObjectId,
        /// For good measure, the input `HEAD^{tree}` that is used as the basis to learn about worktree changes.
        pub head_tree: gix::ObjectId,
        /// The `head_tree`  with the selected worktree changes applied, suitable for being stored in a commit.
        pub wortree: gix::ObjectId,
        /// The tree representing the current changed index, without conflicts, or `None` if there was no change to the index.
        pub index: Option<gix::ObjectId>,
        /// A tree with files in a custom storage format to allow keeping conflicting blobs reachable, along with detailed conflict information
        /// to allow restoring the conflict entries in the index.
        pub index_conflicts: Option<gix::ObjectId>,
        /// The tree representing the reference targets of all references within the *workspace*.
        pub workspace_references: Option<gix::ObjectId>,
        /// The tree representing the reference targets of all references reachable from `HEAD`, so typically `HEAD` itself, and the
        /// target object of the reference it is pointing to.
        pub head_references: Option<gix::ObjectId>,
        /// The tree representing the metadata of all references within the *workspace*.
        pub metadata: Option<gix::ObjectId>,
    }

    pub(super) mod function {
        use super::{Outcome, State};
        use but_core::RefMetadata;
        /// Create a tree that represents the snapshot for the given `selection`, with the basis for everything
        /// being the `head_tree_id` (i.e. the tree to which `HEAD` is ultimately pointing to).
        ///
        /// If `workspace_and_meta` is not `None`, the workspace and metadata to store in the snapshot.
        /// We will only store reference positions, and assume that their commits are safely stored in the reflog to not
        /// be garbage collected. Metadata is only stored for the references that are included in the `workspace`.
        ///
        /// Note that objects will be written into the repository behind `head_tree_id` unless it's configured
        /// to keep everything in memory.
        pub fn create_tree(
            _head_tree_id: gix::Id<'_>,
            _selection: State,
            _workspace_and_meta: Option<(&but_graph::projection::Workspace, &impl RefMetadata)>,
        ) -> anyhow::Result<Outcome> {
            todo!()
        }
    }
}
pub use create_tree::function::create_tree;

/// Utilities related to resolving previously created snapshots.
pub mod resolve_tree {

    /// The information extracted from [`resolve_tree`](function::resolve_tree()).
    pub struct Outcome<'repo> {
        /// The cherry-pick result as merge between the target worktree and the snapshot, **possibly with conflicts**.
        ///
        /// This tree, may be checked out to the working tree, with or without conflicts.
        pub workspace_cherry_pick: gix::merge::tree::Outcome<'repo>,
        /// If an index was stored in the snapshot, this is the reconstructed index, including conflicts.
        pub index: Option<gix::index::State>,
        /// Reference edits that when applied in a transaction will set the workspace back to where it was. Only available
        /// if it was part of the snapshot to begin with.
        pub workspace_references: Option<Vec<gix::refs::transaction::RefEdit>>,
        /// The metadata to be applied to the ref-metadata store.
        pub metadata: Option<MetadataEdits>,
    }

    /// Edits for application via [`but_core::RefMetadata`].
    pub struct MetadataEdits {
        /// The workspace metadata stored in the snapshot.
        pub workspace: (gix::refs::FullName, but_core::ref_metadata::Workspace),
        /// The branch metadata stored in snapshots.
        pub branches: Vec<(gix::refs::FullName, but_core::ref_metadata::Branch)>,
    }

    pub(super) mod function {
        use super::Outcome;

        /// Given the `snapshot_tree` as previously returned via [super::create_tree::Outcome::snapshot_tree], extract data and…
        ///
        /// * …cherry-pick the worktree changes onto the `target_worktree_tree_id`, which is assumed to represent the future working directory state
        ///    and which either contains the worktree changes or *preferably* is the `HEAD^{tree}` as the working directory is clean.
        /// * …reconstruct the index to write into `.git/index`, assuming that the current `.git/index` is clean.
        /// * …produce reference edits to put the workspace refs back into place with.
        /// * …produce metadata that if set will represent the metadata of the entire workspace.
        ///
        /// Note that none of this data is actually manifested in the repository or working tree, they only exists as objects in the Git database,
        /// assuming in-memory objects aren't used in the repository.
        pub fn resolve_tree<'repo>(
            _snapshot_tree: gix::Id<'_>,
            _target_worktree_tree_id: gix::ObjectId,
        ) -> anyhow::Result<Outcome<'_>> {
            todo!()
        }
    }
}
pub use resolve_tree::function::resolve_tree;

/// Utilities for associating snapshot-trees with commits and additional metadata.
mod commit {
    use anyhow::anyhow;
    use but_core::RefMetadata;
    use serde::Serialize;
    use std::fmt;
    use std::fmt::{Display, Formatter};
    use std::str::FromStr;

    /// A commit representing a snapshot, along with metadata.
    #[expect(dead_code)]
    pub struct Commit<'repo> {
        /// The id of the commit that was used for accessing its metadata.
        id: gix::Id<'repo>,
        /// The fully decoded commit.
        inner: gix::objs::Commit,
    }

    /// Represents a key value pair stored in a snapshot, like `key: value\n`
    /// Using the git trailer format (<https://git-scm.com/docs/git-interpret-trailers>)
    #[derive(Debug, PartialEq, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CommitTrailer {
        /// Trailer key.
        pub key: String,
        /// Trailer value.
        pub value: String,
    }

    impl Display for CommitTrailer {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            let escaped_value = self.value.replace('\n', "\\n");
            write!(f, "{}: {}", self.key, escaped_value)
        }
    }

    impl FromStr for CommitTrailer {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
            let parts: Vec<&str> = s.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid trailer format, expected `key: value`"));
            }
            let unescaped_value = parts[1].trim().replace("\\n", "\n");
            Ok(Self {
                key: parts[0].trim().to_string(),
                value: unescaped_value,
            })
        }
    }

    /// Metadata attached to [`Commit`]s holding snapshots.
    pub struct CommitMetadata {
        /// The name of the operation that created the commit.
        /// This is an internal string.
        pub operation: String,
        /// The title of the commit for user consumption, typically created using information from `trailers`.
        pub title: String,
        /// Properties to be stored with the commit.
        pub trailers: Vec<CommitTrailer>,
    }

    /// Given a `snapshot_tree` as created by [`super::create_tree()`], associate it with the stash of `ref_name`.
    /// If a stash already exists, put it on top, with a new commit to carry `metadata`.
    pub fn create_stash_commit<'repo>(
        _snapshot_tree: gix::Id<'repo>,
        _ref_name: &gix::refs::FullNameRef,
        _metadata: CommitMetadata,
    ) -> anyhow::Result<Commit<'repo>> {
        todo!()
    }

    /// List all stash commits available for `ref_name`, with the top-most (most recent) first, and the oldest one last.
    pub fn list_stash_commits<'repo>(
        _repo: &'repo gix::Repository,
        _ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Vec<Commit<'repo>>> {
        todo!()
    }

    /// List all references for which a stash is available.
    /// Note that these might not actually exist in the `repo`, for instance if the actual reference was renamed.
    pub fn list_stash_references(_repo: &gix::Repository) -> Vec<gix::refs::FullName> {
        todo!()
    }

    /// Remove the top-most stash from the top of `ref_name` and write back all changes.
    /// Just like Git, write merge conflicts and update the index, possibly update refs and metadata.
    // TODO: should there be a dry-run version of this to know if it will conflict or not? Who likes having merge-conflicts
    //       in the worktree and no stash afterwards? Needs separate method to avoid touching refs.
    pub fn pop_stash_commit(
        _repo: &gix::Repository,
        _ref_name: &gix::refs::FullNameRef,
        _meta: &mut impl RefMetadata,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
pub use commit::{
    Commit, CommitMetadata, CommitTrailer, create_stash_commit, list_stash_commits,
    list_stash_references, pop_stash_commit,
};
