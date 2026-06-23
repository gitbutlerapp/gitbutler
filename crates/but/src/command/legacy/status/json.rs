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

use std::collections::HashMap;

use anyhow::Context as _;
use but_graph::SegmentIndex;
use but_workspace::ref_info::LocalCommit;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    command::legacy::status::FilesStatusFlag,
    id::{RemoteCommitWithId, SegmentWithId, WorkspaceCommitWithId},
};

use super::StatusContext;

/// JSON output for the `but status` command
/// This represents the status of the GitButler "workspace".
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceStatus {
    /// Represents uncommitted changes that are not assigned to any stack
    uncommitted_changes: Vec<FileChange>,
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
    /// List of upstream commits (only populated when requested with --upstream flag)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_commits: Option<Vec<Commit>>,
}

impl WorkspaceStatus {
    pub fn new(
        uncommitted_changes: Vec<FileChange>,
        stacks: Vec<Stack>,
        merge_base: Commit,
        upstream_state: UpstreamState,
    ) -> Self {
        Self {
            uncommitted_changes,
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
    /// The CI status checks associated with this branch, including pending, passing, and failing checks.
    /// This is only populated when CI information is available for the branch (for example, when the
    /// repository is configured with CI and the status has been fetched); otherwise it will be `None`.
    ci: Option<Ci>,
    /// The merge status of the branch with upstream, indicating whether it can be cleanly integrated.
    /// This is only populated when `but status --upstream` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_status: Option<MergeStatus>,
}

/// The aggregated status of CI checks associated with a branch.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ci {
    /// Titles of CI checks that are currently pending or still running
    pub pending_check_titles: Vec<String>,
    /// Titles of CI checks that have completed successfully
    pub passing_check_titles: Vec<String>,
    /// Titles of CI checks that have completed with a failure
    pub failing_check_titles: Vec<String>,
    /// Overall execution status of the CI checks (whether checks are still running or all are complete)
    pub status: CiStatus,
    /// Overall result of the completed CI checks (pass, fail, or unknown), independent of whether checks are still running
    pub conclusion: CiConclusion,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CiStatus {
    /// All CI checks have finished running, regardless of whether they passed or failed.
    Complete,
    /// At least one CI check is still running or has not started yet.
    InProgress,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CiConclusion {
    /// At least one required CI check failed or reported an error.
    Failure,
    /// All required CI checks completed successfully.
    Success,
    /// The overall CI outcome is not known, for example because no checks ran
    /// or the CI provider did not report a final result.
    Unknown,
}

/// The merge status of a branch with the upstream branch
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MergeStatus {
    /// The branch can be cleanly merged or rebased onto the upstream
    Clean,
    /// The branch has already been integrated into the upstream
    Integrated,
    /// The branch has conflicts with the upstream
    Conflicted {
        /// Whether the branch can be rebased (despite conflicts)
        rebasable: bool,
    },
    /// The branch has no commits
    Empty,
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

impl From<Vec<but_forge::CiCheck>> for Ci {
    fn from(checks: Vec<but_forge::CiCheck>) -> Self {
        let mut pending_check_titles = Vec::new();
        let mut passing_check_titles = Vec::new();
        let mut failing_check_titles = Vec::new();
        let mut overall_conclusion = CiConclusion::Unknown;

        for check in checks {
            match check.status {
                but_forge::CiStatus::InProgress => {
                    pending_check_titles.push(check.name);
                }
                but_forge::CiStatus::Queued => {
                    pending_check_titles.push(check.name);
                }
                but_forge::CiStatus::Complete { conclusion, .. } => match conclusion {
                    but_forge::CiConclusion::Success => {
                        passing_check_titles.push(check.name);
                    }
                    but_forge::CiConclusion::Failure => {
                        failing_check_titles.push(check.name);
                    }
                    _ => {
                        // Other conclusions (e.g., Neutral, Skipped, Cancelled, TimedOut,
                        // ActionRequired) are not treated as passing or failing.
                    }
                },
                but_forge::CiStatus::Unknown => {
                    // Intentionally ignore checks with unknown status: they are not included in any
                    // of the *_check_titles lists and do not affect overall status/conclusion.
                }
            }
        }

        let overall_status = if !pending_check_titles.is_empty() {
            CiStatus::InProgress
        } else {
            CiStatus::Complete
        };

        if !failing_check_titles.is_empty() {
            overall_conclusion = CiConclusion::Failure;
        } else if !pending_check_titles.is_empty() {
            overall_conclusion = CiConclusion::Unknown;
        } else if !passing_check_titles.is_empty() {
            overall_conclusion = CiConclusion::Success;
        }

        Ci {
            pending_check_titles,
            passing_check_titles,
            failing_check_titles,
            status: overall_status,
            conclusion: overall_conclusion,
        }
    }
}

impl Branch {
    #[allow(clippy::too_many_arguments)]
    pub fn from_branch_details(
        repo: &gix::Repository,
        cli_id: String,
        segment: SegmentWithId,
        push_statuses_by_segment_id: &HashMap<SegmentIndex, but_workspace::ui::PushStatus>,
        local_commits_by_id: &HashMap<gix::ObjectId, LocalCommit>,
        remote_commits_by_id: &HashMap<gix::ObjectId, but_workspace::ref_info::Commit>,
        review_id: Option<String>,
        show_files: FilesStatusFlag,
        ci: Option<Vec<but_forge::CiCheck>>,
        merge_status: Option<MergeStatus>,
    ) -> anyhow::Result<Self> {
        let commits = segment
            .workspace_commits
            .iter()
            .map(|c| {
                Commit::from_local_commit(
                    repo,
                    c.short_id.clone(),
                    c.clone(),
                    local_commits_by_id,
                    show_files,
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let upstream_commits = segment
            .remote_commits
            .iter()
            .filter_map(|c| {
                Commit::from_remote_commit(
                    c.short_id.clone(),
                    c.clone(),
                    remote_commits_by_id,
                    None,
                )
                .transpose()
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let push_status = push_statuses_by_segment_id
            .get(&segment.inner.id)
            .copied()
            .unwrap_or_else(|| {
                eprintln!("warning: head_info does not have segment that graph has");
                but_workspace::ui::PushStatus::CompletelyUnpushed
            });
        Ok(Branch {
            cli_id,
            name: segment.branch_name().unwrap_or_default().to_string(),
            commits,
            upstream_commits,
            branch_status: push_status.into(),
            review_id,
            ci: ci.map(Ci::from),
            merge_status,
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
        repo: &gix::Repository,
        cli_id: String,
        commit: WorkspaceCommitWithId,
        local_commits_by_id: &HashMap<gix::ObjectId, LocalCommit>,
        show_files: FilesStatusFlag,
    ) -> anyhow::Result<Self> {
        let changes = if show_files.show_files_for(commit.inner.id) {
            Some(
                commit
                    .tree_changes_using_repo(repo)?
                    .into_iter()
                    .map(|tree_change| {
                        FileChange::from_tree_change(tree_change.short_id, tree_change.inner.into())
                    })
                    .collect(),
            )
        } else {
            None
        };

        let commit = &local_commits_by_id
            .get(&commit.commit_id())
            .context("BUG: head_info does not have local commit that graph has")?
            .inner;
        Ok(Commit {
            cli_id,
            commit_id: commit.id.to_string(),
            created_at: gix_time_to_rfc3339(&commit.author.time),
            message: commit.message.to_string(),
            author_name: commit.author.name.to_string(),
            author_email: commit.author.email.to_string(),
            conflicted: Some(commit.has_conflicts),
            // TODO: populate but_workspace::ref_info::LocalCommit with the
            // Gerrit URL
            review_id: None,
            changes,
        })
    }
    pub fn from_remote_commit(
        cli_id: String,
        commit: RemoteCommitWithId,
        remote_commits_by_id: &HashMap<gix::ObjectId, but_workspace::ref_info::Commit>,
        changes: Option<Vec<FileChange>>,
    ) -> anyhow::Result<Option<Self>> {
        let Some(commit) = remote_commits_by_id.get(&commit.commit_id()) else {
            // This was filtered out because there is a corresponding local
            // commit, so don't show it.
            return Ok(None);
        };
        Ok(Some(Commit {
            cli_id,
            commit_id: commit.id.to_string(),
            created_at: gix_time_to_rfc3339(&commit.author.time),
            message: commit.message.to_string(),
            author_name: commit.author.name.to_string(),
            author_email: commit.author.email.to_string(),
            conflicted: None,
            review_id: None,
            changes,
        }))
    }
    /// A commit not obtained from a stack. `IdMap` does not know
    /// about this commit, so it will not have a CLI ID.
    pub fn from_upstream_commit(
        commit: but_workspace::ui::UpstreamCommit,
        changes: Option<Vec<FileChange>>,
    ) -> Self {
        Commit {
            cli_id: String::new(),
            commit_id: commit.id.to_string(),
            created_at: i128_to_rfc3339(commit.authored_at),
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

pub(crate) fn gix_time_to_rfc3339(time: &gix::date::Time) -> String {
    let seconds = time.seconds;

    DateTime::<Utc>::from_timestamp(seconds, 0)
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
    repo: &gix::Repository,
    segment: &SegmentWithId,
    status_ctx: &StatusContext,
) -> anyhow::Result<Branch> {
    let cli_id = segment.short_id.clone();

    let review_numbers = crate::command::legacy::forge::review::get_review_numbers(
        &segment.branch_name().unwrap_or_default().to_string(),
        &segment.pr_number(),
        &status_ctx.review_map,
    );
    let review_id = {
        review_numbers
            .split_whitespace()
            .next()
            .map(|s| s.to_string())
    };

    let ci = segment
        .branch_name()
        .and_then(|name| status_ctx.ci_map.get(&name.to_string()).cloned());

    let merge_status = segment.branch_name().and_then(|name| {
        status_ctx
            .branch_merge_statuses
            .get(&name.to_string())
            .map(|status| match status {
                gitbutler_branch_actions::upstream_integration::BranchStatus::SafelyUpdatable => {
                    MergeStatus::Clean
                }
                gitbutler_branch_actions::upstream_integration::BranchStatus::Integrated => {
                    MergeStatus::Integrated
                }
                gitbutler_branch_actions::upstream_integration::BranchStatus::Conflicted {
                    rebasable,
                } => MergeStatus::Conflicted {
                    rebasable: *rebasable,
                },
                gitbutler_branch_actions::upstream_integration::BranchStatus::Empty => {
                    MergeStatus::Empty
                }
            })
    });

    Branch::from_branch_details(
        repo,
        cli_id,
        segment.clone(),
        &status_ctx.push_statuses_by_segment_id,
        &status_ctx.local_commits_by_id,
        &status_ctx.remote_commits_by_id,
        review_id,
        status_ctx.flags.show_files,
        ci,
        merge_status,
    )
}

/// Build the complete WorkspaceStatus JSON structure.
pub(super) fn build_workspace_status_json(
    status_ctx: &StatusContext,
    repo: &gix::Repository,
) -> anyhow::Result<WorkspaceStatus> {
    let mut json_stacks = Vec::new();
    let mut json_uncommitted_changes = Vec::new();

    for (stack_id, (stack_with_id, assignments)) in &status_ctx.stack_details {
        if stack_id.is_none() {
            json_uncommitted_changes =
                convert_file_assignments(assignments, &status_ctx.worktree_changes);
        } else if let (Some(stack_id), Some(stack_with_id)) = (stack_id, stack_with_id) {
            let stack_cli_id = status_ctx
                .id_map
                .resolve_stack(*stack_id)
                .map(|id| id.to_short_string())
                .unwrap_or_else(|| "unknown".to_string());

            let json_assigned_changes =
                convert_file_assignments(assignments, &status_ctx.worktree_changes);

            let json_branches = stack_with_id
                .segments
                .iter()
                .map(|segment| convert_branch_to_json(repo, segment, status_ctx))
                .collect::<anyhow::Result<Vec<_>>>()?;

            let stack = Stack::new(stack_cli_id, json_assigned_changes, json_branches);
            json_stacks.push(stack);
        }
    }

    // Create a Commit object for the merge base.
    let base_commit = repo.find_commit(status_ctx.common_merge_base_data.commit_id)?;
    let base_commit_decoded = base_commit.decode()?;
    let author: but_workspace::ui::Author = base_commit_decoded.author()?.into();

    let merge_base_commit = Commit::from_upstream_commit(
        but_workspace::ui::UpstreamCommit {
            id: status_ctx.common_merge_base_data.commit_id,
            authored_at: status_ctx.common_merge_base_data.authored_at,
            committed_at: status_ctx.common_merge_base_data.committed_at,
            message: status_ctx.common_merge_base_data.message.clone().into(),
            author,
            // This is a synthetic upstream commit used only to reuse
            // `Commit::from_upstream_commit()`. Legacy status JSON does not
            // expose change-ids, so dropping it here is fine.
            change_id: None,
        },
        None,
    );

    let upstream_state_json = if let Some(upstream) = &status_ctx.upstream_state {
        // Create a Commit object for the latest upstream commit
        let upstream_commit = repo.find_commit(upstream.commit_id)?;
        let upstream_commit_decoded = upstream_commit.decode()?;
        let upstream_author: but_workspace::ui::Author = upstream_commit_decoded.author()?.into();

        let latest_commit = Commit::from_upstream_commit(
            but_workspace::ui::UpstreamCommit {
                id: upstream.commit_id,
                authored_at: upstream.authored_at,
                committed_at: upstream.committed_at,
                message: upstream.message.clone().into(),
                author: upstream_author,
                // This is a synthetic upstream commit used only to reuse
                // `Commit::from_upstream_commit()`. Legacy status JSON does not
                // expose change-ids, so dropping it here is fine.
                change_id: None,
            },
            None,
        );

        let last_fetched = status_ctx
            .last_fetched_ms
            .map(|ts| i128_to_rfc3339(ts as i128));

        // Populate upstream_commits if show_upstream flag is set and base_branch is available.
        let upstream_commits = if status_ctx.flags.show_upstream {
            status_ctx.base_branch.as_ref().and_then(|bb| {
                if bb.upstream_commits.is_empty() {
                    None
                } else {
                    let commits: anyhow::Result<Vec<Commit>> = bb
                        .upstream_commits
                        .iter()
                        .map(|remote_commit| {
                            let commit_oid = gix::ObjectId::from_hex(remote_commit.id.as_bytes())?;
                            // Convert the author manually since there's no From impl between the two Author types
                            let author = but_workspace::ui::Author {
                                name: remote_commit.author.name.clone(),
                                email: remote_commit.author.email.clone(),
                                gravatar_url: remote_commit.author.gravatar_url.clone(),
                            };
                            Ok(Commit::from_upstream_commit(
                                but_workspace::ui::UpstreamCommit {
                                    id: commit_oid,
                                    authored_at: remote_commit.authored_at as i128,
                                    committed_at: remote_commit.committed_at as i128,
                                    message: remote_commit.description.clone().into(),
                                    author,
                                    // This is a synthetic upstream commit used
                                    // only to reuse `Commit::from_upstream_commit()`.
                                    // Legacy status JSON does not expose
                                    // change-ids, so dropping it here is fine.
                                    change_id: None,
                                },
                                None,
                            ))
                        })
                        .collect();
                    commits.ok()
                }
            })
        } else {
            None
        };

        UpstreamState {
            behind: upstream.behind_count,
            latest_commit,
            last_fetched,
            upstream_commits,
        }
    } else {
        // When up to date, use the merge base as the latest commit
        let last_fetched = status_ctx
            .last_fetched_ms
            .map(|ts| i128_to_rfc3339(ts as i128));

        UpstreamState {
            behind: 0,
            latest_commit: merge_base_commit.clone(),
            last_fetched,
            upstream_commits: None,
        }
    };

    Ok(WorkspaceStatus::new(
        json_uncommitted_changes,
        json_stacks,
        merge_base_commit,
        upstream_state_json,
    ))
}
