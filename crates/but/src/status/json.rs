//! Definitions for the JSON output of the but CLI
//! The types defined here specific to the CLI output format, hence they are not to be exported.
//!
//! The focus of this serialization format is:
//! - Simplicity: The output should be easy to read and understand.
//! - CLI focus: Included are idnentifiers that only make sense in the context of the but CLI.
//! - Stability: The format should not have breaking changes.
//!
//! Non-goals:
//! - Completeness: The output structures do not include all the data that the internal but-api has.

use chrono::{DateTime, Utc};
use serde::Serialize;

/// JSON output for the `but status` command
/// This represents the status of the GitButler "workspace".
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceStatus {
    /// Represents uncommitted changes that are not assigned to any stack
    unassigned_changes: Vec<FileChange>,
    /// The stacks that are applied in the current workspace
    stacks: Vec<Stack>,
    /// The most recent common merge base between all applied stacks and the target upstream branch
    merge_base: Commit,
    /// Information about how ahead the target upstream branch is compared to the merge base
    upstream_state: UpstreamState,
}

/// Represents the state of the upstream branch compared to the merge base
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpstreamState {
    /// The number of commits the upstream is ahead of the merge base
    pub behind: usize,
    /// The latest commit on the upstream branch
    pub latest_commit: Commit,
    /// Timestamp of when the upstream branch was last fetched, in RFC3339 format
    pub last_fetched: Option<String>,
}

impl WorkspaceStatus {
    pub fn new(
        unassigned_changes: Vec<FileChange>,
        stacks: Vec<Stack>,
        merge_base: Commit,
        upstream_state: UpstreamState,
    ) -> Self {
        Self {
            unassigned_changes,
            stacks,
            merge_base,
            upstream_state,
        }
    }
}

/// Represents a stack of branches applied in the current workspace
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stack {
    /// A unique ID specific to the current state of the workspace, to be used by other CLI operations (e.g `rub`)
    cli_id: String,
    /// Represents uncommitted changes assigned to this stack
    assigned_changes: Vec<FileChange>,
    /// The branches that are part of this stack, newest first
    branches: Vec<Branch>,
}

impl Stack {
    pub fn new(cli_id: String, assigned_changes: Vec<FileChange>, branches: Vec<Branch>) -> Self {
        Self {
            cli_id,
            assigned_changes,
            branches,
        }
    }
}

/// Represents a branch in the GitButler workspace
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Branch {
    /// A unique ID specific to the current state of the workspace, to be used by other CLI operations (e.g `rub`)
    cli_id: String,
    /// The name of the branch, e.g. "feature/add-new-api"
    name: String,
    /// The commits that are part of this branch, newest first
    commits: Vec<Commit>,
    /// The commits that are only at the upstream of this branch, newest first
    upstream_commits: Vec<Commit>,
    /// Represents the status of the branch with respect to the upstream
    branch_status: BranchStatus,
    /// If but status was invoked with --review and if the branch has an associated review ID (eg. PR number), it will be present here
    review_id: Option<String>,
}

/// The status of a branch with respect to its upstream
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BranchStatus {
    /// Can push, but there are no changes to be pushed
    NothingToPush,
    /// Can push. This is the case when there are local changes that can be pushed to the remote.
    UnpushedCommits,
    /// Can push, but requires a force push to the remote because commits were rewritten.
    UnpushedCommitsRequiringForce,
    /// Completely unpushed - there is no remote tracking branch so Git never interacted with the remote.
    CompletelyUnpushed,
    /// Fully integrated, no changes to push.
    Integrated,
}

/// A commit that is in the GitButler workspace
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Commit {
    /// A unique ID specific to the current state of the workspace, to be used by other CLI operations (e.g `rub`)
    cli_id: String,
    /// The commit ID (SHA-1 or SHA-256 depending on the repository configuration)
    commit_id: String,
    /// Timestamp of when the commit was created in format "YYYY-MM-DD HH:MM:SS +ZZZZ"
    created_at: String,
    /// The commit message
    message: String,
    /// The name of the commit author
    author_name: String,
    /// The email of the commit author
    author_email: String,
    /// Wheter the commit is in a conflicted state. Only applicable to local commits (and not to upstream commits)
    conflicted: Option<bool>,
    /// If but status was invoked with --review and if the commit has an associated review ID (eg. Gerrit review number), it will be present here
    review_id: Option<String>,
    /// If but status was invoked with --files, the list of file changes in this commit will be present here
    changes: Option<Vec<FileChange>>,
}

/// A change to a file in the repository
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileChange {
    /// A unique ID specific to the current state of the workspace, to be used by other CLI operations (e.g `rub`)
    cli_id: String,
    /// The file path, UTF-8 encoded (note - this can be lossy for some Operating Systems)
    file_path: String,
    /// The type of change that happened to the file
    change_type: ChangeType,
}

/// The type of change that happened to a file
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ChangeType {
    /// The file was newly added (it was not tracked before)
    Added,
    /// The file was deleted
    Removed,
    /// The file was modified
    Modified,
    /// The file was renamed
    Renamed,
}

impl Branch {
    pub fn from_branch_details(
        cli_id: String,
        branch: but_workspace::ui::BranchDetails,
        review_id: Option<String>,
        show_files: bool,
        project_id: gitbutler_project::ProjectId,
    ) -> anyhow::Result<Self> {
        let commits = branch
            .commits
            .iter()
            .map(|c| {
                Commit::from_local_commit(
                    crate::id::CliId::commit(c.id).to_string(),
                    c.clone(),
                    show_files,
                    project_id,
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let upstream_commits = branch
            .upstream_commits
            .iter()
            .map(|c| {
                Commit::from_upstream_commit(
                    crate::id::CliId::commit(c.id).to_string(),
                    c.clone(),
                    None,
                )
            })
            .collect();

        Ok(Branch {
            cli_id,
            name: branch.name.to_string(),
            commits,
            upstream_commits,
            branch_status: branch.push_status.into(),
            review_id,
        })
    }
}

impl FileChange {
    pub fn from_tree_change(cli_id: String, tree_change: but_core::ui::TreeChange) -> Self {
        FileChange {
            cli_id,
            file_path: tree_change.path.to_string(),
            change_type: tree_change.status.into(),
        }
    }
}

impl Commit {
    pub fn from_local_commit(
        cli_id: String,
        commit: but_workspace::ui::Commit,
        show_files: bool,
        project_id: gitbutler_project::ProjectId,
    ) -> anyhow::Result<Self> {
        let changes = if show_files {
            let commit_details =
                but_api::legacy::diff::commit_details(project_id, commit.id.into())?;
            Some(
                commit_details
                    .changes
                    .changes
                    .into_iter()
                    .map(|change| {
                        let cli_id =
                            crate::id::CliId::committed_file(&change.path.to_string(), commit.id);
                        FileChange::from_tree_change(cli_id.to_string(), change)
                    })
                    .collect(),
            )
        } else {
            None
        };

        Ok(Commit {
            cli_id,
            commit_id: commit.id.to_string(),
            created_at: i128_to_rfc3339(commit.created_at),
            message: commit.message.to_string(),
            author_name: commit.author.name,
            author_email: commit.author.email,
            conflicted: Some(commit.has_conflicts),
            review_id: commit.gerrit_review_url,
            changes,
        })
    }
    pub fn from_upstream_commit(
        cli_id: String,
        commit: but_workspace::ui::UpstreamCommit,
        changes: Option<Vec<FileChange>>,
    ) -> Self {
        Commit {
            cli_id,
            commit_id: commit.id.to_string(),
            created_at: i128_to_rfc3339(commit.created_at),
            message: commit.message.to_string(),
            author_name: commit.author.name,
            author_email: commit.author.email,
            conflicted: None,
            review_id: None,
            changes,
        }
    }
}

impl From<but_workspace::ui::PushStatus> for BranchStatus {
    fn from(status: but_workspace::ui::PushStatus) -> Self {
        match status {
            but_workspace::ui::PushStatus::NothingToPush => BranchStatus::NothingToPush,
            but_workspace::ui::PushStatus::UnpushedCommits => BranchStatus::UnpushedCommits,
            but_workspace::ui::PushStatus::UnpushedCommitsRequiringForce => {
                BranchStatus::UnpushedCommitsRequiringForce
            }
            but_workspace::ui::PushStatus::CompletelyUnpushed => BranchStatus::CompletelyUnpushed,
            but_workspace::ui::PushStatus::Integrated => BranchStatus::Integrated,
        }
    }
}

impl From<but_core::ui::TreeStatus> for ChangeType {
    fn from(status: but_core::ui::TreeStatus) -> Self {
        match status {
            but_core::ui::TreeStatus::Addition { .. } => ChangeType::Added,
            but_core::ui::TreeStatus::Deletion { .. } => ChangeType::Removed,
            but_core::ui::TreeStatus::Modification { .. } => ChangeType::Modified,
            but_core::ui::TreeStatus::Rename { .. } => ChangeType::Renamed,
        }
    }
}

pub(crate) fn i128_to_rfc3339(ts_millis: i128) -> String {
    let seconds = (ts_millis / 1000) as i64;
    let nanos = ((ts_millis % 1000) * 1_000_000) as u32;

    DateTime::<Utc>::from_timestamp(seconds, nanos)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}
