use anyhow::Result;
use but_oxidize::ObjectIdExt;
use but_serde::BStringForFrontend;
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr as _};
use serde::Serialize;

use crate::author::Author;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommit {
    pub id: String,
    pub description: BStringForFrontend,
    pub created_at: u128,
    pub author: Author,
    pub change_id: Option<String>,
    #[serde(with = "but_serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub conflicted: bool,
}

pub(crate) fn commit_to_remote_commit(commit: &gix::Commit) -> Result<RemoteCommit> {
    let parent_ids = commit.parent_ids().map(|id| id.to_git2()).collect();
    Ok(RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message_bstr().into(),
        created_at: commit.time()?.seconds.try_into().unwrap(),
        author: commit.author()?.into(),
        change_id: commit.change_id().map(|c| c.to_string()),
        parent_ids,
        conflicted: commit.is_conflicted(),
    })
}
