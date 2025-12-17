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

use crate::CliId;
use but_api::diff::ComputeLineStats;
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
    /// Whether the commit is in a conflicted state. Only applicable to local commits (and not to upstream commits)
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
        id_map: &mut crate::IdMap,
    ) -> anyhow::Result<Self> {
        let commits = branch
            .commits
            .iter()
            .map(|c| {
                Commit::from_local_commit(
                    CliId::Commit(c.id).to_short_string(),
                    c.clone(),
                    show_files,
                    project_id,
                    id_map,
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let upstream_commits = branch
            .upstream_commits
            .iter()
            .map(|c| {
                Commit::from_upstream_commit(CliId::Commit(c.id).to_short_string(), c.clone(), None)
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
        id_map: &mut crate::IdMap,
    ) -> anyhow::Result<Self> {
        let changes = if show_files {
            // TODO: we should get the `ctx` as parameter.
            let ctx = but_ctx::Context::new_from_legacy_project_id(project_id)?;
            let commit_details: but_api::diff::json::CommitDetails =
                but_api::diff::commit_details(&ctx, commit.id, ComputeLineStats::No)?.into();
            Some(
                commit_details
                    .changes
                    .into_iter()
                    .map(|change| {
                        let cli_id = id_map.resolve_file_changed_in_commit_or_unassigned(
                            commit.id,
                            change.path.as_ref(),
                        );
                        FileChange::from_tree_change(cli_id.to_short_string(), change)
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

/// Convert file assignments to JSON FileChange objects
fn convert_file_assignments(
    assignments: &[super::assignment::FileAssignment],
    worktree_changes: &[but_core::ui::TreeChange],
) -> Vec<FileChange> {
    assignments
        .iter()
        .filter_map(|fa| {
            let cli_id = fa.assignments[0].cli_id.to_string();
            let change = worktree_changes.iter().find(|c| c.path_bytes == fa.path)?;
            Some(FileChange::from_tree_change(cli_id, change.clone()))
        })
        .collect()
}

/// Convert a BranchDetails to the JSON Branch type
fn convert_branch_to_json(
    branch: &but_workspace::ui::BranchDetails,
    review: bool,
    show_files: bool,
    project_id: gitbutler_project::ProjectId,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    id_map: &mut crate::IdMap,
) -> anyhow::Result<Branch> {
    let cli_id = id_map
        .resolve_branch(branch.name.as_ref())
        .to_short_string();

    let review_id = if review {
        crate::command::legacy::forge::review::get_review_numbers(
            &branch.name.to_string(),
            &branch.pr_number,
            review_map,
        )
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
    } else {
        None
    };

    Branch::from_branch_details(
        cli_id.to_string(),
        branch.clone(),
        review_id,
        show_files,
        project_id,
        id_map,
    )
}

/// Build the complete WorkspaceStatus JSON structure
#[expect(clippy::too_many_arguments)]
pub(super) fn build_workspace_status_json(
    original_stack_details: &[(
        Option<gitbutler_stack::StackId>,
        Option<but_workspace::ui::StackDetails>,
    )],
    stack_details: &[super::StackEntry],
    worktree_changes: &[but_core::ui::TreeChange],
    common_merge_base: &super::CommonMergeBase,
    upstream_state: &Option<super::UpstreamState>,
    last_fetched_ms: Option<u128>,
    review_map: &std::collections::HashMap<String, Vec<but_forge::ForgeReview>>,
    show_files: bool,
    review: bool,
    project_id: gitbutler_project::ProjectId,
    repo: &gix::Repository,
    id_map: &mut crate::IdMap,
) -> anyhow::Result<WorkspaceStatus> {
    let mut json_stacks = Vec::new();
    let mut json_unassigned_changes = Vec::new();

    for (idx, (stack_id, original_details)) in original_stack_details.iter().enumerate() {
        let (_, (_, assignments)) = &stack_details[idx];

        if stack_id.is_none() {
            json_unassigned_changes = convert_file_assignments(assignments, worktree_changes);
        } else if let Some(details) = original_details {
            let stack_cli_id = details
                .branch_details
                .first()
                .map(|b| id_map.resolve_branch(b.name.as_ref()).to_short_string())
                .unwrap_or_else(|| "unknown".to_string());

            let json_assigned_changes = convert_file_assignments(assignments, worktree_changes);

            let json_branches = details
                .branch_details
                .iter()
                .map(|branch| {
                    convert_branch_to_json(
                        branch, review, show_files, project_id, review_map, id_map,
                    )
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            let stack = Stack::new(stack_cli_id, json_assigned_changes, json_branches);
            json_stacks.push(stack);
        }
    }

    // Create a Commit object for the merge base
    // We use the author signature from the commit we decoded earlier
    let base_commit = repo.find_commit(common_merge_base.commit_id)?;
    let base_commit_decoded = base_commit.decode()?;
    let author: but_workspace::ui::Author = base_commit_decoded.author()?.into();

    let cli_id = CliId::Commit(common_merge_base.commit_id).to_short_string();
    let merge_base_commit = Commit::from_upstream_commit(
        cli_id,
        but_workspace::ui::UpstreamCommit {
            id: common_merge_base.commit_id,
            created_at: common_merge_base.created_at,
            message: common_merge_base.message.clone().into(),
            author,
        },
        None,
    );

    let upstream_state_json = if let Some(upstream) = upstream_state {
        // Create a Commit object for the latest upstream commit
        let upstream_commit = repo.find_commit(upstream.commit_id)?;
        let upstream_commit_decoded = upstream_commit.decode()?;
        let upstream_author: but_workspace::ui::Author = upstream_commit_decoded.author()?.into();

        let cli_id = CliId::Commit(upstream.commit_id).to_short_string();
        let latest_commit = Commit::from_upstream_commit(
            cli_id,
            but_workspace::ui::UpstreamCommit {
                id: upstream.commit_id,
                created_at: upstream.created_at,
                message: upstream.message.clone().into(),
                author: upstream_author,
            },
            None,
        );

        let last_fetched = last_fetched_ms.map(|ts| i128_to_rfc3339(ts as i128));

        UpstreamState {
            behind: upstream.behind_count,
            latest_commit,
            last_fetched,
        }
    } else {
        // When up to date, use the merge base as the latest commit
        let last_fetched = last_fetched_ms.map(|ts| i128_to_rfc3339(ts as i128));

        UpstreamState {
            behind: 0,
            latest_commit: merge_base_commit.clone(),
            last_fetched,
        }
    };

    Ok(WorkspaceStatus::new(
        json_unassigned_changes,
        json_stacks,
        merge_base_commit,
        upstream_state_json,
    ))
}
