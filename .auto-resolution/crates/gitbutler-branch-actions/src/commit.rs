use crate::{author::Author, dependencies::CommitDependencies};
use anyhow::{anyhow, Result};
use bstr::ByteSlice as _;
use gitbutler_cherry_pick::ConflictedTreeKey;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_repo::rebase::ConflictEntries;
use gitbutler_serde::BStringForFrontend;
use gitbutler_stack::{Stack, StackId};
use serde::Serialize;

// this is the struct that maps to the view `Commit` type in Typescript
// it is derived from walking the git commits between the `Branch.head` commit
// and the `Target.sha` commit, or, everything that is uniquely committed to
// the virtual branch we assign it to. an array of them are returned as part of
// the `VirtualBranch` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchCommit {
    #[serde(with = "gitbutler_serde::oid")]
    pub id: git2::Oid,
    pub description: BStringForFrontend,
    pub created_at: u128,
    pub author: Author,
    /// If the commit is remote AND ONLY REMOTE
    pub is_remote: bool,
    /// If the commit is both local and remote
    pub is_local_and_remote: bool,
    /// If the commit is integrated. A commit may be local_and_remote, local,
    /// or remote, and be integrated.
    pub is_integrated: bool,
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub branch_id: StackId,
    pub change_id: Option<String>,
    pub is_signed: bool,
    pub conflicted: bool,
    /// The id of the remote commit from which this one was copied, as identified by
    /// having equal author, committer, and commit message.
    /// This is used by the frontend similar to the `change_id` to group matching commits.
    #[serde(with = "gitbutler_serde::oid_opt")]
    pub copied_from_remote_id: Option<git2::Oid>,
    /// Represents the remote commit id of this patch.
    /// This field is set if:
    ///   - The commit has been pushed
    ///   - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the commit has been rebased or updated.
    ///
    /// Note: This makes both the `is_remote` and `copied_from_remote_id` fields redundant, but they are kept for compatibility.
    #[serde(with = "gitbutler_serde::oid_opt")]
    pub remote_commit_id: Option<git2::Oid>,
    pub conflicted_files: ConflictEntries,
    // Dependency tracking
    // ---
    /// Commits depended on.
    #[serde(default, with = "gitbutler_serde::oid_vec")]
    pub dependencies: Vec<git2::Oid>,
    /// Commits that depend on this commit.
    #[serde(default, with = "gitbutler_serde::oid_vec")]
    pub reverse_dependencies: Vec<git2::Oid>,
    /// Hashes of uncommitted hunks files that depend on this commit.
    pub dependent_diffs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_to_vbranch_commit(
    repo: &git2::Repository,
    stack: &Stack,
    commit: &git2::Commit,
    is_integrated: bool,
    is_remote: bool,
    is_local_and_remote: bool,
    copied_from_remote_id: Option<git2::Oid>,
    remote_commit_id: Option<git2::Oid>,
    commit_dependencies: CommitDependencies,
) -> Result<VirtualBranchCommit> {
    let timestamp = u128::try_from(commit.time().seconds())?;
    let message = commit.message_bstr().to_owned();

    let parent_ids: Vec<git2::Oid> = commit
        .parents()
        .map(|c| {
            let c: git2::Oid = c.id();
            c
        })
        .collect::<Vec<_>>();

    let conflicted_files = if commit.is_conflicted() {
        let conflict_files_string = commit.tree()?;
        let conflict_files_string = conflict_files_string
            .get_name(&ConflictedTreeKey::ConflictFiles)
            .ok_or_else(|| anyhow!("conflict files not found"))?;
        let conflict_files_string = repo
            .find_blob(conflict_files_string.id())?
            .content()
            .to_str_lossy()
            .to_string();
        toml::from_str::<ConflictEntries>(&conflict_files_string).unwrap_or_default()
    } else {
        Default::default()
    };

    let CommitDependencies {
        dependencies,
        reverse_dependencies: inverse_dependencies,
        dependent_diffs,
    } = commit_dependencies;

    let commit = VirtualBranchCommit {
        id: commit.id(),
        created_at: timestamp * 1000,
        author: commit.author().into(),
        description: message.into(),
        is_remote,
        is_integrated,
        is_local_and_remote,
        parent_ids,
        branch_id: stack.id,
        change_id: commit.change_id(),
        is_signed: commit.is_signed(),
        conflicted: commit.is_conflicted(),
        copied_from_remote_id,
        remote_commit_id,
        conflicted_files,
        dependencies,
        reverse_dependencies: inverse_dependencies,
        dependent_diffs,
    };

    Ok(commit)
}
