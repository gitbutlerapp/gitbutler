use anyhow::{Context, Result};
use bstr::BString;
use gitbutler_branch::{Branch, BranchId};
use gitbutler_command_context::ProjectRepository;
use gitbutler_commit::commit_ext::CommitExt;
use serde::Serialize;

use crate::{
    author::Author,
    file::{list_virtual_commit_files, VirtualBranchFile},
};

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
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub id: git2::Oid,
    #[serde(serialize_with = "gitbutler_serde::serde::as_string_lossy")]
    pub description: BString,
    pub created_at: u128,
    pub author: Author,
    pub is_remote: bool,
    pub files: Vec<VirtualBranchFile>,
    pub is_integrated: bool,
    #[serde(with = "gitbutler_serde::serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub branch_id: BranchId,
    pub change_id: Option<String>,
    pub is_signed: bool,
}

pub(crate) fn commit_to_vbranch_commit(
    repository: &ProjectRepository,
    branch: &Branch,
    commit: &git2::Commit,
    is_integrated: bool,
    is_remote: bool,
) -> Result<VirtualBranchCommit> {
    let timestamp = u128::try_from(commit.time().seconds())?;
    let message = commit.message_bstr().to_owned();

    let files =
        list_virtual_commit_files(repository, commit).context("failed to list commit files")?;

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
        description: message,
        is_remote,
        files,
        is_integrated,
        parent_ids,
        branch_id: branch.id,
        change_id: commit.change_id(),
        is_signed: commit.is_signed(),
    };

    Ok(commit)
}
