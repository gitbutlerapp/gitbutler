use std::fmt::Display;

use bstr::ByteSlice;
use but_ctx::Context;
use but_db::DbHandle;
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr as _};
use serde::{Deserialize, Serialize};

use crate::parse::PushOutput;

pub mod parse;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum PushFlag {
    Wip,
    Ready,
    Private,
    Hashtag(String),
    Topic(String),
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PushFlag);

impl Display for PushFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushFlag::Wip => write!(f, "wip"),
            PushFlag::Ready => write!(f, "ready"),
            PushFlag::Private => write!(f, "private"),
            PushFlag::Hashtag(tag) => write!(f, "t={tag}"),
            PushFlag::Topic(topic) => write!(f, "topic={topic}"),
        }
    }
}

pub use but_core::commit::write::{GerritChangeId, set_gerrit_trailers as set_trailers};

pub fn record_push_metadata_with_context(
    ctx: &Context,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let mappings = mappings(&repo, candidate_ids, push_output)?;
    let mut db = ctx.db.get_cache_mut()?;
    let mut trans = db.transaction()?;

    for mapping in mappings {
        let existing = trans.gerrit_metadata().get(&mapping.change_id)?;
        let now = chrono::Utc::now().naive_utc();
        let commit_id_str = mapping.commit_id.to_string();

        match existing {
            Some(existing_meta) => {
                // Check if commit_id has changed
                if existing_meta.commit_id != commit_id_str {
                    // Update the entry with new commit_id and updated_at
                    let updated_meta = but_db::GerritMeta {
                        change_id: mapping.change_id,
                        commit_id: commit_id_str,
                        review_url: mapping.review_url,
                        created_at: existing_meta.created_at, // Keep original creation time
                        updated_at: now,
                    };
                    trans.gerrit_metadata_mut().update(updated_meta)?;
                }
                // If commit_id matches, do nothing
            }
            None => {
                // Create new entry
                let new_meta = but_db::GerritMeta {
                    change_id: mapping.change_id,
                    commit_id: commit_id_str,
                    review_url: mapping.review_url,
                    created_at: now,
                    updated_at: now,
                };
                trans.gerrit_metadata_mut().insert(new_meta)?;
            }
        }
    }
    trans.commit()?;

    Ok(())
}

pub fn record_push_metadata(
    repo: &gix::Repository,
    db: &mut DbHandle,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<()> {
    let mappings = mappings(repo, candidate_ids, push_output)?;
    let mut trans = db.transaction()?;

    for mapping in mappings {
        let existing = trans.gerrit_metadata().get(&mapping.change_id)?;
        let now = chrono::Utc::now().naive_utc();
        let commit_id_str = mapping.commit_id.to_string();

        match existing {
            Some(existing_meta) => {
                // Check if commit_id has changed
                if existing_meta.commit_id != commit_id_str {
                    // Update the entry with new commit_id and updated_at
                    let updated_meta = but_db::GerritMeta {
                        change_id: mapping.change_id,
                        commit_id: commit_id_str,
                        review_url: mapping.review_url,
                        created_at: existing_meta.created_at, // Keep original creation time
                        updated_at: now,
                    };
                    trans.gerrit_metadata_mut().update(updated_meta)?;
                }
                // If commit_id matches, do nothing
            }
            None => {
                // Create new entry
                let new_meta = but_db::GerritMeta {
                    change_id: mapping.change_id,
                    commit_id: commit_id_str,
                    review_url: mapping.review_url,
                    created_at: now,
                    updated_at: now,
                };
                trans.gerrit_metadata_mut().insert(new_meta)?;
            }
        }
    }
    trans.commit()?;

    Ok(())
}

struct ChangeIdMapping {
    commit_id: gix::ObjectId,
    change_id: String,
    review_url: String,
}

fn mappings(
    repo: &gix::Repository,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<Vec<ChangeIdMapping>> {
    let mut mappings = vec![];
    let host = gerrit_host(repo);
    for id in candidate_ids {
        let commit = repo.find_commit(id)?;
        let msg = commit.message_bstr().to_string();
        let title = msg.lines().next().unwrap_or_default();

        let change_id_review_url = push_output
            .changes
            .iter()
            .find(|c| c.commit_title == title)
            .and_then(|c| {
                commit
                    .change_id()
                    .map(|change_id| (change_id, c.url.clone()))
            });

        if let Some((change_id, review_url)) = change_id_review_url {
            mappings.push(ChangeIdMapping {
                commit_id: id,
                change_id: change_id.to_string(),
                review_url,
            });
        } else if let (Some(change_id), Some(host)) = (commit.change_id(), host.as_ref()) {
            // Fallback: generate review URL if we have a change ID and a host
            let gerrit_change_id = GerritChangeId::from(&change_id);
            let review_url = format!("https://{host}/q/{gerrit_change_id}");
            mappings.push(ChangeIdMapping {
                commit_id: id,
                change_id: change_id.to_string(),
                review_url,
            });
        }
    }
    Ok(mappings)
}

fn gerrit_host(repo: &gix::Repository) -> Option<String> {
    let name = repo.remote_default_name(gix::remote::Direction::Push);
    let name = name
        .as_ref()
        .map(|n| n.as_ref())
        .unwrap_or(b"origin".as_bstr());
    let remote = repo.find_remote(name).ok()?;
    let url = remote
        .url(gix::remote::Direction::Push)
        .or_else(|| remote.url(gix::remote::Direction::Fetch))?;
    url.host().map(|h| h.to_string())
}
