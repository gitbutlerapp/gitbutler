use std::vec;

use anyhow::Result;
use gitbutler_branch::BranchUpdateRequest;
use gitbutler_project::{access::WorktreeWritePermission, Project};
use gitbutler_reference::ReferenceName;
use gitbutler_stack::Stack;

use super::entry::Trailer;
use crate::{
    entry::{OperationKind, SnapshotDetails},
    oplog::OplogExt,
};

pub trait SnapshotExt {
    fn snapshot_branch_unapplied(
        &self,
        snapshot_tree: git2::Oid,
        result: Result<&ReferenceName, &anyhow::Error>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;

    fn snapshot_commit_undo(
        &self,
        snapshot_tree: git2::Oid,
        result: Result<&(), &anyhow::Error>,
        commit_sha: git2::Oid,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;

    fn snapshot_commit_creation(
        &self,
        snapshot_tree: git2::Oid,
        error: Option<&anyhow::Error>,
        commit_message: String,
        sha: Option<git2::Oid>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;

    fn snapshot_branch_creation(
        &self,
        branch_name: String,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
    fn snapshot_branch_deletion(
        &self,
        branch_name: String,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
    fn snapshot_branch_update(
        &self,
        snapshot_tree: git2::Oid,
        old_stack: &Stack,
        update: &BranchUpdateRequest,
        error: Option<&anyhow::Error>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
    fn snapshot_create_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
    fn snapshot_remove_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
    fn snapshot_update_dependent_branch_name(
        &self,
        new_branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()>;
}

/// Snapshot functionality
impl SnapshotExt for Project {
    fn snapshot_branch_unapplied(
        &self,
        snapshot_tree: git2::Oid,
        result: Result<&ReferenceName, &anyhow::Error>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let result = result.map(|s| Some(s.to_string()));
        let details = SnapshotDetails::new(OperationKind::UnapplyBranch)
            .with_trailers(result_trailer(result, "name".to_string()));
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }
    fn snapshot_commit_undo(
        &self,
        snapshot_tree: git2::Oid,
        result: Result<&(), &anyhow::Error>,
        commit_sha: git2::Oid,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let result = result.map(|_| Some(commit_sha.to_string()));
        let details = SnapshotDetails::new(OperationKind::UndoCommit)
            .with_trailers(result_trailer(result, "sha".to_string()));
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }
    fn snapshot_commit_creation(
        &self,
        snapshot_tree: git2::Oid,
        error: Option<&anyhow::Error>,
        commit_message: String,
        sha: Option<git2::Oid>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::CreateCommit).with_trailers(
            [
                vec![
                    Trailer {
                        key: "message".to_string(),
                        value: commit_message,
                    },
                    Trailer {
                        key: "sha".to_string(),
                        value: sha.map(|sha| sha.to_string()).unwrap_or_default(),
                    },
                ],
                error_trailer(error),
            ]
            .concat(),
        );
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }
    fn snapshot_branch_creation(
        &self,
        branch_name: String,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::CreateBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }
    fn snapshot_branch_deletion(
        &self,
        branch_name: String,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::DeleteBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name.to_string(),
            }]);

        self.create_snapshot(details, perm)?;
        Ok(())
    }
    fn snapshot_branch_update(
        &self,
        snapshot_tree: git2::Oid,
        old_stack: &Stack,
        update: &BranchUpdateRequest,
        error: Option<&anyhow::Error>,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details = if update.ownership.is_some() {
            SnapshotDetails::new(OperationKind::MoveHunk).with_trailers(
                [
                    vec![Trailer {
                        key: "name".to_string(),
                        value: old_stack.name.to_string(),
                    }],
                    error_trailer(error),
                ]
                .concat(),
            )
        } else if let Some(name) = update.name.as_deref() {
            SnapshotDetails::new(OperationKind::UpdateBranchName).with_trailers(
                [
                    vec![
                        Trailer {
                            key: "before".to_string(),
                            value: old_stack.name.clone(),
                        },
                        Trailer {
                            key: "after".to_string(),
                            value: name.to_owned(),
                        },
                    ],
                    error_trailer(error),
                ]
                .concat(),
            )
        } else if update.notes.is_some() {
            SnapshotDetails::new(OperationKind::UpdateBranchNotes)
        } else if let Some(order) = update.order {
            SnapshotDetails::new(OperationKind::ReorderBranches).with_trailers(
                [
                    vec![
                        Trailer {
                            key: "before".to_string(),
                            value: old_stack.order.to_string(),
                        },
                        Trailer {
                            key: "after".to_string(),
                            value: order.to_string(),
                        },
                    ],
                    error_trailer(error),
                ]
                .concat(),
            )
        } else if let Some(_selected_for_changes) = update.selected_for_changes {
            SnapshotDetails::new(OperationKind::SelectDefaultVirtualBranch).with_trailers(
                [
                    vec![
                        Trailer {
                            key: "before".to_string(),
                            value: old_stack
                                .selected_for_changes
                                .unwrap_or_default()
                                .to_string(),
                        },
                        Trailer {
                            key: "after".to_string(),
                            value: old_stack.name.clone(),
                        },
                    ],
                    error_trailer(error),
                ]
                .concat(),
            )
        } else if let Some(upstream) = update.upstream.as_deref() {
            SnapshotDetails::new(OperationKind::UpdateBranchRemoteName).with_trailers(
                [
                    vec![
                        Trailer {
                            key: "before".to_string(),
                            value: old_stack
                                .upstream
                                .as_ref()
                                .map(|r| r.to_string())
                                .unwrap_or_default(),
                        },
                        Trailer {
                            key: "after".to_string(),
                            value: upstream.to_owned(),
                        },
                    ],
                    error_trailer(error),
                ]
                .concat(),
            )
        } else {
            SnapshotDetails::new(OperationKind::GenericBranchUpdate)
        };
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }
    fn snapshot_create_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::CreateDependentBranch).with_trailers(vec![
                Trailer {
                    key: "name".to_string(),
                    value: branch_name.to_string(),
                },
            ]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }
    fn snapshot_remove_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::RemoveDependentBranch).with_trailers(vec![
                Trailer {
                    key: "name".to_string(),
                    value: branch_name.to_string(),
                },
            ]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }
    fn snapshot_update_dependent_branch_name(
        &self,
        new_branch_name: &str,
        perm: &mut WorktreeWritePermission,
    ) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::UpdateDependentBranchName).with_trailers(vec![
                Trailer {
                    key: "name".to_string(),
                    value: new_branch_name.to_string(),
                },
            ]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }
}

fn result_trailer(result: Result<Option<String>, &anyhow::Error>, key: String) -> Vec<Trailer> {
    match result {
        Ok(v) => {
            if let Some(v) = v {
                vec![Trailer {
                    key,
                    value: v.clone(),
                }]
            } else {
                vec![]
            }
        }
        Err(error) => vec![Trailer {
            key: "error".to_string(),
            value: error.to_string(),
        }],
    }
}

fn error_trailer(error: Option<&anyhow::Error>) -> Vec<Trailer> {
    error
        .map(|e| {
            vec![Trailer {
                key: "error".to_string(),
                value: e.to_string(),
            }]
        })
        .unwrap_or_default()
}
