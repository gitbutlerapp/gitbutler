use crate::{
    author::Author,
    file::{list_virtual_commit_files, VirtualBranchFile},
};
use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_serde::BStringForFrontend;
use gitbutler_stack::{Branch, BranchId};
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
    pub is_remote: bool,
    pub files: Vec<VirtualBranchFile>,
    pub is_integrated: bool,
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub branch_id: BranchId,
    pub change_id: Option<String>,
    pub is_signed: bool,
    pub conflicted: bool,
    /// The id of the remote commit from which this one was copied, as identified by
    /// having equal author, committer, and commit message.
    /// This is used by the frontend similar to the `change_id` to group matching commits.
    #[serde(with = "gitbutler_serde::oid_opt")]
    pub copied_from_remote_id: Option<git2::Oid>,
}

pub(crate) fn commit_to_vbranch_commit(
    ctx: &CommandContext,
    branch: &Branch,
    commit: &git2::Commit,
    is_integrated: bool,
    is_remote: bool,
    copied_from_remote_id: Option<git2::Oid>,
) -> Result<VirtualBranchCommit> {
    let timestamp = u128::try_from(commit.time().seconds())?;
    let message = commit.message_bstr().to_owned();

    let files = list_virtual_commit_files(ctx, commit).context("failed to list commit files")?;

    let parent_ids: Vec<git2::Oid> = commit
        .parents()
        .map(|c| {
            let c: git2::Oid = c.id();
            c
        })
        .collect::<Vec<_>>();

    let commit = VirtualBranchCommit {
        id: commit.id(),
        created_at: timestamp * 1000,
        author: commit.author().into(),
        description: message.into(),
        is_remote,
        files,
        is_integrated,
        parent_ids,
        branch_id: branch.id,
        change_id: commit.change_id(),
        is_signed: commit.is_signed(),
        conflicted: commit.is_conflicted(),
        copied_from_remote_id,
    };

    Ok(commit)
}
