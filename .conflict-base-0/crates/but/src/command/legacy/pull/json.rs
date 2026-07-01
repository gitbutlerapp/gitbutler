//! JSON output structures for `but pull` commands.

use serde::Serialize;

/// JSON output for `but pull --check`
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullCheckOutput {
    pub base_branch: BaseBranchInfo,
    pub upstream_commits: UpstreamInfo,
    pub branch_statuses: Vec<BranchStatusInfo>,
    pub up_to_date: bool,
    pub has_worktree_conflicts: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BaseBranchInfo {
    pub name: String,
    pub remote_name: String,
    pub base_sha: String,
    pub current_sha: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpstreamInfo {
    pub count: usize,
    pub commits: Vec<UpstreamCommit>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpstreamCommit {
    pub id: String,
    pub description: String,
    pub author_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BranchStatusInfo {
    pub name: String,
    pub status: String,
    pub rebasable: Option<bool>,
}
