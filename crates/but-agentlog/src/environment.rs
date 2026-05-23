use std::{
    any::Any,
    ops::{Deref, DerefMut},
    path::Path,
};

use bstr::{BStr, ByteSlice};
use but_core::{
    RefMetadata, RepositoryExt, TreeChange, TreeStatus, open_repo_for_merging,
    ref_metadata::{Branch, ValueInfo, Workspace},
};
use but_meta::VirtualBranchesTomlMetadata;
use serde::Serialize;

const MAX_PATHS: usize = 256;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Debug, Serialize)]
pub(crate) struct EnvironmentSnapshot {
    snapshot_status: SnapshotStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_kind: Option<SnapshotErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    worktree: Option<PathList>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stacks: Vec<StackSnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum SnapshotErrorKind {
    #[serde(rename = "context_unavailable")]
    Context,
    #[serde(rename = "workspace_unavailable")]
    Workspace,
    #[serde(rename = "diff_unavailable")]
    Diff,
}

#[derive(Debug, Serialize)]
struct PathList {
    files: Vec<String>,
    file_count: usize,
    files_truncated: bool,
}

#[derive(Debug, Serialize)]
struct StackSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    stack_id: Option<String>,
    branches: Vec<BranchSnapshot>,
}

#[derive(Debug, Serialize)]
struct BranchSnapshot {
    key: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<ReviewTarget>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct ReviewTarget {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pull_request: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_id: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct ObservedTargets {
    branches: Vec<BranchTarget>,
    reviews: Vec<ReviewTarget>,
    changes: Vec<ChangeTarget>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct BranchTarget {
    key: String,
    name: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct ChangeTarget {
    key: String,
    change_id: String,
}

#[derive(Debug)]
pub(crate) struct EnvironmentObservation {
    environment: EnvironmentSnapshot,
    observed_targets: ObservedTargets,
}

impl EnvironmentObservation {
    pub(crate) fn environment(&self) -> &EnvironmentSnapshot {
        &self.environment
    }

    pub(crate) fn observed_targets(&self) -> &ObservedTargets {
        &self.observed_targets
    }

    pub(crate) fn snapshot_status(&self) -> SnapshotStatus {
        self.environment.snapshot_status
    }

    #[cfg(test)]
    pub(crate) fn from_observed_targets_for_testing(observed_targets: ObservedTargets) -> Self {
        Self {
            environment: EnvironmentSnapshot {
                snapshot_status: SnapshotStatus::Complete,
                error_kind: None,
                worktree: Some(PathList::empty()),
                stacks: Vec::new(),
            },
            observed_targets,
        }
    }
}

pub(crate) fn capture_environment(repo_path: &Path) -> EnvironmentObservation {
    let Ok(repo) = open_repo_for_merging(repo_path) else {
        return EnvironmentObservation {
            environment: failed_environment(SnapshotErrorKind::Context),
            observed_targets: ObservedTargets::default(),
        };
    };

    environment_from_parts(
        worktree_paths(&repo).map_err(|_| SnapshotErrorKind::Diff),
        workspace_snapshot(&repo).map_err(|_| SnapshotErrorKind::Workspace),
    )
}

fn environment_from_parts(
    worktree: Result<PathList, SnapshotErrorKind>,
    workspace: Result<WorkspaceObservation, SnapshotErrorKind>,
) -> EnvironmentObservation {
    match (worktree, workspace) {
        (Ok(worktree), Ok(workspace)) => {
            let snapshot_status = if workspace.error_kind.is_some() {
                SnapshotStatus::Partial
            } else {
                SnapshotStatus::Complete
            };
            EnvironmentObservation {
                environment: EnvironmentSnapshot {
                    snapshot_status,
                    error_kind: workspace.error_kind,
                    worktree: Some(worktree),
                    stacks: workspace.stacks,
                },
                observed_targets: workspace.observed_targets,
            }
        }
        (Ok(worktree), Err(error_kind)) => EnvironmentObservation {
            environment: EnvironmentSnapshot {
                snapshot_status: SnapshotStatus::Partial,
                error_kind: Some(error_kind),
                worktree: Some(worktree),
                stacks: Vec::new(),
            },
            observed_targets: ObservedTargets::default(),
        },
        (Err(error_kind), Ok(workspace)) => EnvironmentObservation {
            environment: if workspace.has_useful_observation() {
                EnvironmentSnapshot {
                    snapshot_status: SnapshotStatus::Partial,
                    error_kind: workspace.error_kind.or(Some(error_kind)),
                    worktree: None,
                    stacks: workspace.stacks,
                }
            } else {
                failed_environment(workspace.error_kind.unwrap_or(error_kind))
            },
            observed_targets: workspace.observed_targets,
        },
        (Err(error_kind), Err(_)) => EnvironmentObservation {
            environment: failed_environment(error_kind),
            observed_targets: ObservedTargets::default(),
        },
    }
}

fn failed_environment(error_kind: SnapshotErrorKind) -> EnvironmentSnapshot {
    EnvironmentSnapshot {
        snapshot_status: SnapshotStatus::Failed,
        error_kind: Some(error_kind),
        worktree: None,
        stacks: Vec::new(),
    }
}

struct WorkspaceObservation {
    stacks: Vec<StackSnapshot>,
    observed_targets: ObservedTargets,
    error_kind: Option<SnapshotErrorKind>,
}

impl WorkspaceObservation {
    fn has_useful_observation(&self) -> bool {
        !self.stacks.is_empty()
            || !self.observed_targets.branches.is_empty()
            || !self.observed_targets.reviews.is_empty()
            || !self.observed_targets.changes.is_empty()
    }
}

fn worktree_paths(repo: &gix::Repository) -> anyhow::Result<PathList> {
    let changes = but_core::diff::worktree_changes_no_renames(repo)?;
    Ok(path_list(paths_from_changes(changes.changes)))
}

fn workspace_snapshot(repo: &gix::Repository) -> anyhow::Result<WorkspaceObservation> {
    let metadata_path = repo.gitbutler_storage_path()?.join("virtual_branches.toml");
    let legacy_metadata_exists = legacy_metadata_exists(&metadata_path);
    if legacy_metadata_exists
        && let Ok(meta) = VirtualBranchesTomlMetadata::from_path_read_only(&metadata_path)
    {
        return workspace_snapshot_with_meta(repo, &meta);
    }

    let mut observation = workspace_snapshot_with_meta(repo, &EmptyRefMetadata)?;
    if legacy_metadata_exists && observation.error_kind.is_none() {
        observation.error_kind = Some(SnapshotErrorKind::Workspace);
    }
    Ok(observation)
}

fn legacy_metadata_exists(metadata_path: &Path) -> bool {
    metadata_path.is_file()
        || metadata_path
            .parent()
            .is_some_and(|storage_path| storage_path.join("but.sqlite").is_file())
}

fn workspace_snapshot_with_meta(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
) -> anyhow::Result<WorkspaceObservation> {
    let info = but_workspace::head_info(
        repo,
        meta,
        but_workspace::ref_info::Options {
            expensive_commit_info: false,
            ..Default::default()
        },
    )?;

    let mut observed_targets = ObservedTargets::default();
    let mut stacks = Vec::new();
    for stack in info.stacks {
        let stack_id = stack.id.map(|id| id.to_string());
        let mut branches = Vec::new();
        for segment in stack.segments {
            let Some(ref_info) = segment.ref_info.as_ref() else {
                continue;
            };
            let full_ref_name = ref_info.ref_name.to_string();
            let branch = BranchTarget {
                key: format!("ref:{full_ref_name}"),
                name: ref_info.ref_name.shorten().to_string(),
            };
            let review = review_target(&branch.key, segment.metadata.as_ref());
            for commit in &segment.commits {
                let change_id = commit.change_id.as_ref().map(ToString::to_string);
                if let Some(change_id) = &change_id {
                    observed_targets.changes.push(ChangeTarget {
                        key: format!("gitbutler-change:{change_id}"),
                        change_id: change_id.clone(),
                    });
                }
            }

            observed_targets.branches.push(branch.clone());
            if let Some(review) = review.clone() {
                observed_targets.reviews.push(review);
            }
            branches.push(BranchSnapshot {
                key: branch.key,
                name: branch.name,
                review,
            });
        }
        if !branches.is_empty() {
            stacks.push(StackSnapshot { stack_id, branches });
        }
    }

    observed_targets.sort_and_dedup();
    Ok(WorkspaceObservation {
        stacks,
        observed_targets,
        error_kind: None,
    })
}

fn review_target(branch_key: &str, metadata: Option<&Branch>) -> Option<ReviewTarget> {
    let review = &metadata?.review;
    if let Some(review_id) = &review.review_id {
        return Some(ReviewTarget {
            key: format!("gitbutler-review:{review_id}"),
            pull_request: review.pull_request,
            review_id: Some(review_id.clone()),
        });
    }
    review.pull_request.map(|pull_request| ReviewTarget {
        key: format!("pull-request:{branch_key}#{pull_request}"),
        pull_request: Some(pull_request),
        review_id: None,
    })
}

fn paths_from_changes(changes: impl IntoIterator<Item = TreeChange>) -> Vec<String> {
    let mut paths = Vec::new();
    for change in changes {
        paths.push(path_to_string(change.path.as_bstr()));
        if let TreeStatus::Rename { previous_path, .. } = change.status {
            paths.push(path_to_string(previous_path.as_bstr()));
        }
    }
    paths
}

fn path_list(mut files: Vec<String>) -> PathList {
    files.sort();
    files.dedup();
    let file_count = files.len();
    let files_truncated = file_count > MAX_PATHS;
    files.truncate(MAX_PATHS);
    PathList {
        files,
        file_count,
        files_truncated,
    }
}

fn path_to_string(path: &BStr) -> String {
    path.to_str_lossy().into_owned()
}

#[cfg(test)]
impl PathList {
    fn empty() -> Self {
        Self {
            files: Vec::new(),
            file_count: 0,
            files_truncated: false,
        }
    }
}

impl ObservedTargets {
    pub(crate) fn branch_keys(&self) -> impl Iterator<Item = &str> {
        self.branches.iter().map(|target| target.key.as_str())
    }

    pub(crate) fn review_keys(&self) -> impl Iterator<Item = &str> {
        self.reviews.iter().map(|target| target.key.as_str())
    }

    pub(crate) fn change_keys(&self) -> impl Iterator<Item = &str> {
        self.changes.iter().map(|target| target.key.as_str())
    }

    fn sort_and_dedup(&mut self) {
        self.branches.sort_by(|lhs, rhs| lhs.key.cmp(&rhs.key));
        self.branches.dedup_by(|lhs, rhs| lhs.key == rhs.key);
        self.reviews.sort_by(|lhs, rhs| lhs.key.cmp(&rhs.key));
        self.reviews.dedup_by(|lhs, rhs| lhs.key == rhs.key);
        self.changes.sort_by(|lhs, rhs| lhs.key.cmp(&rhs.key));
        self.changes.dedup_by(|lhs, rhs| lhs.key == rhs.key);
    }

    #[cfg(test)]
    pub(crate) fn from_index_keys_for_testing(
        branch_key: &str,
        review_key: &str,
        change_key: &str,
    ) -> Self {
        Self {
            branches: vec![BranchTarget {
                key: branch_key.to_owned(),
                name: branch_key.to_owned(),
            }],
            reviews: vec![ReviewTarget {
                key: review_key.to_owned(),
                pull_request: None,
                review_id: Some(review_key.to_owned()),
            }],
            changes: vec![ChangeTarget {
                key: change_key.to_owned(),
                change_id: change_key.to_owned(),
            }],
        }
    }
}

#[derive(Debug)]
struct EmptyRefMetadata;

struct EmptyRefMetadataHandle<T> {
    ref_name: gix::refs::FullName,
    value: T,
}

impl<T> AsRef<gix::refs::FullNameRef> for EmptyRefMetadataHandle<T> {
    fn as_ref(&self) -> &gix::refs::FullNameRef {
        self.ref_name.as_ref()
    }
}

impl<T> Deref for EmptyRefMetadataHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for EmptyRefMetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> ValueInfo for EmptyRefMetadataHandle<T> {
    fn is_default(&self) -> bool {
        true
    }
}

impl RefMetadata for EmptyRefMetadata {
    type Handle<T> = EmptyRefMetadataHandle<T>;

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<(gix::refs::FullName, Box<dyn Any>)>> {
        std::iter::empty()
    }

    fn workspace(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<Workspace>> {
        Ok(EmptyRefMetadataHandle {
            ref_name: ref_name.to_owned(),
            value: Workspace::default(),
        })
    }

    fn branch(&self, ref_name: &gix::refs::FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
        Ok(EmptyRefMetadataHandle {
            ref_name: ref_name.to_owned(),
            value: Branch::default(),
        })
    }

    fn set_workspace(&mut self, _value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_branch(&mut self, _value: &Self::Handle<Branch>) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove(&mut self, _ref_name: &gix::refs::FullNameRef) -> anyhow::Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_from_parts_classifies_partial_and_failed_observations() {
        for (worktree, workspace, status, error_kind, has_worktree) in [
            (
                Err(SnapshotErrorKind::Diff),
                Err(SnapshotErrorKind::Workspace),
                SnapshotStatus::Failed,
                Some(SnapshotErrorKind::Diff),
                false,
            ),
            (
                Ok(path_list(vec!["src/lib.rs".to_owned()])),
                Err(SnapshotErrorKind::Workspace),
                SnapshotStatus::Partial,
                Some(SnapshotErrorKind::Workspace),
                true,
            ),
            (
                Ok(PathList::empty()),
                Ok(empty_workspace(Some(SnapshotErrorKind::Diff))),
                SnapshotStatus::Partial,
                Some(SnapshotErrorKind::Diff),
                true,
            ),
            (
                Err(SnapshotErrorKind::Diff),
                Ok(empty_workspace(None)),
                SnapshotStatus::Failed,
                Some(SnapshotErrorKind::Diff),
                false,
            ),
        ] {
            let observation = environment_from_parts(worktree, workspace);

            assert_eq!(observation.environment.snapshot_status, status);
            assert_eq!(observation.environment.error_kind, error_kind);
            assert_eq!(observation.environment.worktree.is_some(), has_worktree);
        }
    }

    fn empty_workspace(error_kind: Option<SnapshotErrorKind>) -> WorkspaceObservation {
        WorkspaceObservation {
            stacks: Vec::new(),
            observed_targets: ObservedTargets::default(),
            error_kind,
        }
    }
}
