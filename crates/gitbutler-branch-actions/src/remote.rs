use anyhow::Result;
use but_serde::BStringForFrontend;
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr as _};
use serde::Serialize;

use but_workspace::ui::Author;

#[but_api_macros::but_transport]
#[derive(Clone, PartialEq)]
pub struct RemoteCommit {
    pub id: String,
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub description: BStringForFrontend,
    pub created_at: u128,
    pub author: Author,
    pub change_id: Option<String>,
    #[serde(with = "but_serde::object_id_vec")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id_vec")
    )]
    pub parent_ids: Vec<gix::ObjectId>,
    pub conflicted: bool,
}

pub(crate) fn commit_to_remote_commit(commit: &gix::Commit) -> Result<RemoteCommit> {
    let parent_ids = commit.parent_ids().map(|id| id.detach()).collect();
    Ok(RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message_bstr().into(),
        created_at: u128::try_from(commit.time()?.seconds).unwrap() * 1000,
        author: commit.author()?.into(),
        change_id: commit.change_id().map(|c| c.to_string()),
        parent_ids,
        conflicted: commit.is_conflicted(),
    })
}
