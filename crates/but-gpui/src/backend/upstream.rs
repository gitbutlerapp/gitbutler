//! The target branch's history: what came in that the workspace doesn't have yet, the pull
//! request each commit landed, and when the project last fetched.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;

use super::{CommitId, open};

#[derive(Debug, Clone)]
pub struct TargetCommit {
    pub id: CommitId,
    pub title: String,
    /// Seconds since the epoch.
    pub time: i64,
    /// The pull request it landed, as "#12" and its title.
    pub review: Option<(String, String)>,
    /// The workspace doesn't have it yet.
    pub incoming: bool,
}

/// Up to `limit` of the target's commits, newest first, after `after` or from its tip;
/// and whether there are older ones.
pub fn target_commits(
    path: &Path,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let ctx = open(path)?;
    let page = but_api::target_commits::workspace_target_commits(
        &ctx,
        after.map(|id| but_api::json::HexHash(id.0)),
        Some(limit),
    )?;
    let commits = page
        .commits
        .into_iter()
        .map(|target| {
            let commit = target.commit;
            TargetCommit {
                id: CommitId(commit.id),
                title: commit
                    .message
                    .lines()
                    .next()
                    .map(|line| line.to_str_lossy().into_owned())
                    .unwrap_or_default(),
                time: (commit.committed_at / 1000) as i64,
                review: target.review.map(|review| {
                    (
                        format!("{}{}", review.unit_symbol, review.number),
                        review.title,
                    )
                }),
                incoming: !target.in_workspace,
            }
        })
        .collect();
    Ok((commits, page.has_more))
}

/// When the project last fetched, in seconds since the epoch, and why the last attempt
/// failed if it did.
pub fn fetch_status(path: &Path) -> Result<(Option<i64>, Option<String>)> {
    let status = but_api::workspace::workspace_fetch_status(&open(path)?)?;
    Ok((
        status.last_successful_ms.map(|ms| (ms / 1000) as i64),
        status.last_error,
    ))
}
