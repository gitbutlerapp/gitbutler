use std::{
    any::Any,
    ops::{Deref, DerefMut},
    path::Path,
};

use bstr::{BStr, ByteSlice};
use but_core::{
    RefMetadata, RepositoryExt, TreeChange, TreeStatus, is_workspace_ref_name,
    open_repo_for_merging,
    ref_metadata::{Branch, ValueInfo, Workspace},
};
use but_meta::VirtualBranchesTomlMetadata;
use gix::prelude::ObjectIdExt as _;
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_PATHS: usize = 256;
// Commit file paths are captured on every turn, so keep the synced GitMeta
// payload bounded to recent branch tips.
const MAX_COMMIT_SNAPSHOTS_PER_BRANCH: usize = 32;

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
    #[serde(rename = "truncated")]
    Truncated,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reviews: Vec<TargetKey>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    commits: Vec<CommitSnapshot>,
}

#[derive(Debug, Serialize)]
struct CommitSnapshot {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<TargetKey>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    file_hashes: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct TargetKey {
    key: String,
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

    #[cfg(test)]
    pub(crate) fn from_branch_commits_for_testing(
        observed_targets: ObservedTargets,
        branches: Vec<TestBranchCommitSnapshot>,
    ) -> Self {
        Self::from_worktree_and_branch_commits_for_testing(&[], observed_targets, branches)
    }

    #[cfg(test)]
    pub(crate) fn from_worktree_and_branch_commits_for_testing(
        worktree_files: &[&str],
        observed_targets: ObservedTargets,
        branches: Vec<TestBranchCommitSnapshot>,
    ) -> Self {
        Self {
            environment: EnvironmentSnapshot {
                snapshot_status: SnapshotStatus::Complete,
                error_kind: None,
                worktree: Some(path_list(
                    worktree_files
                        .iter()
                        .map(|path| (*path).to_owned())
                        .collect(),
                )),
                stacks: vec![StackSnapshot {
                    stack_id: None,
                    branches: branches
                        .into_iter()
                        .map(TestBranchCommitSnapshot::into_branch_snapshot)
                        .collect(),
                }],
            },
            observed_targets,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_partial_diff_for_testing(mut self) -> Self {
        self.environment.snapshot_status = SnapshotStatus::Partial;
        self.environment.error_kind = Some(SnapshotErrorKind::Diff);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_partial_truncated_for_testing(mut self) -> Self {
        self.environment.snapshot_status = SnapshotStatus::Partial;
        self.environment.error_kind = Some(SnapshotErrorKind::Truncated);
        self
    }
}

#[cfg(test)]
pub(crate) struct TestBranchCommitSnapshot {
    pub(crate) branch_key: String,
    pub(crate) review_keys: Vec<String>,
    pub(crate) change_key: Option<String>,
    pub(crate) commit_id: Option<String>,
    pub(crate) files: Vec<String>,
}

#[cfg(test)]
impl TestBranchCommitSnapshot {
    fn into_branch_snapshot(self) -> BranchSnapshot {
        let reviews = self
            .review_keys
            .into_iter()
            .map(|key| TargetKey { key })
            .collect::<Vec<_>>();
        let change = self.change_key.map(|key| TargetKey { key });
        let files = path_list(self.files);
        let commits = self
            .commit_id
            .map(|id| CommitSnapshot {
                id,
                change,
                file_hashes: file_hashes(&files.files),
            })
            .into_iter()
            .collect();
        BranchSnapshot {
            key: self.branch_key.clone(),
            name: self.branch_key,
            reviews,
            commits,
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
        return workspace_snapshot_with_current_branch_fallback(repo, &meta);
    }

    let mut observation = workspace_snapshot_with_current_branch_fallback(repo, &EmptyRefMetadata)?;
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

fn workspace_snapshot_with_current_branch_fallback(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
) -> anyhow::Result<WorkspaceObservation> {
    match workspace_snapshot_with_meta(repo, meta) {
        Ok(mut observation) => {
            if observation.observed_targets.is_empty()
                && let Ok(fallback) = current_branch_targets(repo, meta)
                && !fallback.is_empty()
            {
                observation.observed_targets = fallback;
            }
            Ok(observation)
        }
        Err(error) => match current_branch_targets(repo, meta) {
            Ok(fallback) if !fallback.is_empty() => Ok(WorkspaceObservation {
                stacks: Vec::new(),
                observed_targets: fallback,
                error_kind: Some(SnapshotErrorKind::Workspace),
            }),
            _ => Err(error),
        },
    }
}

fn current_branch_targets(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
) -> anyhow::Result<ObservedTargets> {
    let head = repo.head()?;
    let Some(ref_name) = head.referent_name() else {
        anyhow::bail!("HEAD is detached");
    };
    let full_ref_name = ref_name.to_string();
    if !full_ref_name.starts_with("refs/heads/") {
        anyhow::bail!("HEAD does not point to a local branch");
    }
    if is_workspace_ref_name(ref_name) {
        anyhow::bail!("HEAD points to the GitButler workspace branch");
    }
    let branch = BranchTarget {
        key: format!("ref:{full_ref_name}"),
        name: ref_name.shorten().to_string(),
    };
    let reviews = meta
        .branch_opt(ref_name)
        .ok()
        .flatten()
        .map(|branch_metadata| review_targets(&branch.key, Some(&branch_metadata)))
        .unwrap_or_default();
    let mut observed_targets = ObservedTargets {
        branches: vec![branch],
        reviews,
        changes: Vec::new(),
    };
    observed_targets.sort_and_dedup();
    Ok(observed_targets)
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
    let mut commit_paths_failed = false;
    let mut commit_snapshots_truncated = false;
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
            let reviews = review_targets(&branch.key, segment.metadata.as_ref());
            let mut commits = Vec::new();
            commit_snapshots_truncated |= segment.commits.len() > MAX_COMMIT_SNAPSHOTS_PER_BRANCH;
            for (commit_index, commit) in segment.commits.iter().enumerate() {
                let change = commit.change_id.as_ref().map(|change_id| ChangeTarget {
                    key: format!("gitbutler-change:{change_id}"),
                    change_id: change_id.to_string(),
                });
                if let Some(change) = &change {
                    observed_targets.changes.push(change.clone());
                }
                let commit_change = change.as_ref().map(|change| TargetKey {
                    key: change.key.clone(),
                });
                if commit_index >= MAX_COMMIT_SNAPSHOTS_PER_BRANCH {
                    continue;
                }
                let files = match commit_paths(repo, commit.id) {
                    Ok(files) => files,
                    Err(_) => {
                        commit_paths_failed = true;
                        path_list(Vec::new())
                    }
                };
                commits.push(CommitSnapshot {
                    id: commit.id.to_hex().to_string(),
                    change: commit_change,
                    file_hashes: file_hashes(&files.files),
                });
            }

            observed_targets.branches.push(branch.clone());
            let branch_reviews = reviews
                .iter()
                .map(|review| TargetKey {
                    key: review.key.clone(),
                })
                .collect();
            observed_targets.reviews.extend(reviews.iter().cloned());
            branches.push(BranchSnapshot {
                key: branch.key,
                name: branch.name,
                reviews: branch_reviews,
                commits,
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
        error_kind: if commit_paths_failed {
            Some(SnapshotErrorKind::Diff)
        } else if commit_snapshots_truncated {
            Some(SnapshotErrorKind::Truncated)
        } else {
            None
        },
    })
}

fn review_targets(branch_key: &str, metadata: Option<&Branch>) -> Vec<ReviewTarget> {
    let Some(review) = metadata.map(|metadata| &metadata.review) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    if let Some(review_id) = &review.review_id {
        targets.push(ReviewTarget {
            key: format!("gitbutler-review:{review_id}"),
            pull_request: review.pull_request,
            review_id: Some(review_id.clone()),
        });
    }
    if let Some(pull_request) = review.pull_request {
        targets.push(ReviewTarget {
            key: format!("pull-request:{branch_key}#{pull_request}"),
            pull_request: Some(pull_request),
            review_id: None,
        });
    }
    targets
}

fn commit_paths(repo: &gix::Repository, commit_id: gix::ObjectId) -> anyhow::Result<PathList> {
    let changes = but_core::diff::commit_changes(commit_id.attach(repo))?;
    Ok(path_list(paths_from_changes(changes.into_tree_changes())))
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

fn file_hashes(files: &[String]) -> Vec<String> {
    files.iter().map(|path| path_fingerprint(path)).collect()
}

pub(crate) fn path_fingerprint(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hex::encode(&hasher.finalize()[..16])
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
    fn is_empty(&self) -> bool {
        self.branches.is_empty() && self.reviews.is_empty() && self.changes.is_empty()
    }

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
        Self::from_index_key_sets_for_testing(&[branch_key], &[review_key], &[change_key])
    }

    #[cfg(test)]
    pub(crate) fn from_index_key_sets_for_testing(
        branch_keys: &[&str],
        review_keys: &[&str],
        change_keys: &[&str],
    ) -> Self {
        Self {
            branches: branch_keys
                .iter()
                .map(|key| BranchTarget {
                    key: (*key).to_owned(),
                    name: (*key).to_owned(),
                })
                .collect(),
            reviews: review_keys
                .iter()
                .map(|key| ReviewTarget {
                    key: (*key).to_owned(),
                    pull_request: None,
                    review_id: Some((*key).to_owned()),
                })
                .collect(),
            changes: change_keys
                .iter()
                .map(|key| ChangeTarget {
                    key: (*key).to_owned(),
                    change_id: (*key).to_owned(),
                })
                .collect(),
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
    fn review_targets_include_review_id_and_pull_request() {
        let mut metadata = Branch::default();
        metadata.review.review_id = Some("review-1".to_owned());
        metadata.review.pull_request = Some(42);

        let targets = review_targets("ref:refs/heads/feature", Some(&metadata));

        assert_eq!(
            targets,
            [
                ReviewTarget {
                    key: "gitbutler-review:review-1".to_owned(),
                    pull_request: Some(42),
                    review_id: Some("review-1".to_owned()),
                },
                ReviewTarget {
                    key: "pull-request:ref:refs/heads/feature#42".to_owned(),
                    pull_request: Some(42),
                    review_id: None,
                }
            ]
        );
    }

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
