//! The ability to create a Git representation of diverse 'state' that can be restored at a later time.

/// Structures to call the [create_tree()] function.
pub mod create_tree;
pub use create_tree::function::create_tree;

/// Utilities related to resolving previously created snapshots.
pub mod resolve_tree;
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
            let mut parts = s.splitn(2, ':');
            let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
                return Err(anyhow!("Invalid trailer format, expected `key: value`"));
            };
            let unescaped_value = value.trim().replace("\\n", "\n");
            Ok(Self {
                key: key.trim().to_string(),
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
