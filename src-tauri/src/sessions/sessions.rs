use std::path::Path;

use crate::git::activity;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    // timestamp of when the session was created
    pub start_timestamp_ms: u128,
    // timestamp of when the session was last active
    pub last_timestamp_ms: u128,
    // session branch name
    pub branch: Option<String>,
    // session commit hash
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    // if hash is not set, the session is not saved aka current
    pub hash: Option<String>,
    pub meta: Meta,
    pub activity: Vec<activity::Activity>,
}

impl Session {
    pub fn from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<Self> {
        let tree = commit.tree().with_context(|| {
            format!("failed to get tree from commit {}", commit.id().to_string())
        })?;

        let start_timestamp_ms = read_as_string(repo, &tree, Path::new("session/meta/start"))?
            .parse::<u128>()
            .with_context(|| {
                format!(
                    "failed to parse start timestamp from commit {}",
                    commit.id().to_string()
                )
            })?;

        let logs_path = Path::new("logs/HEAD");
        let activity = match tree.get_path(logs_path).is_ok() {
            true => read_as_string(repo, &tree, logs_path)
                .with_context(|| {
                    format!(
                        "failed to read reflog from commit {}",
                        commit.id().to_string()
                    )
                })?
                .lines()
                .filter_map(|line| activity::parse_reflog_line(line).ok())
                .filter(|activity| activity.timestamp_ms >= start_timestamp_ms)
                .collect::<Vec<activity::Activity>>(),
            false => Vec::new(),
        };

        let branch_path = Path::new("session/meta/branch");
        let session_branch = match tree.get_path(branch_path).is_ok() {
            true => read_as_string(repo, &tree, branch_path)
                .with_context(|| {
                    format!(
                        "failed to read branch name from commit {}",
                        commit.id().to_string()
                    )
                })?
                .into(),
            false => None,
        };

        let commit_path = Path::new("session/meta/commit");
        let session_commit = match tree.get_path(commit_path).is_ok() {
            true => read_as_string(repo, &tree, commit_path)
                .with_context(|| {
                    format!(
                        "failed to read branch name from commit {}",
                        commit.id().to_string()
                    )
                })?
                .into(),
            false => None,
        };

        Ok(Session {
            id: read_as_string(repo, &tree, Path::new("session/meta/id")).with_context(|| {
                format!(
                    "failed to read session id from commit {}",
                    commit.id().to_string()
                )
            })?,
            hash: Some(commit.id().to_string()),
            meta: Meta {
                start_timestamp_ms,
                last_timestamp_ms: read_as_string(repo, &tree, Path::new("session/meta/last"))?
                    .parse::<u128>()
                    .with_context(|| {
                        format!(
                            "failed to parse last timestamp from commit {}",
                            commit.id().to_string()
                        )
                    })?,
                branch: session_branch,
                commit: session_commit,
            },
            activity,
        })
    }
}

pub fn id_from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<String> {
    let tree = commit.tree().unwrap();
    let session_id_path = Path::new("session/meta/id");
    if !tree.get_path(session_id_path).is_ok() {
        return Err(anyhow!("commit does not have a session id"));
    }
    let id = read_as_string(repo, &tree, session_id_path)?;
    return Ok(id);
}

fn read_as_string(repo: &git2::Repository, tree: &git2::Tree, path: &Path) -> Result<String> {
    let tree_entry = tree.get_path(path)?;
    let blob = tree_entry.to_object(repo)?.into_blob().unwrap();
    let contents = String::from_utf8(blob.content().to_vec()).with_context(|| {
        format!(
            "failed to read file {} as string",
            path.to_str().unwrap_or("unknown")
        )
    })?;
    Ok(contents)
}
