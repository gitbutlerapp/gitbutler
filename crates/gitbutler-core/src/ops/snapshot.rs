use std::vec;

use crate::projects::Project;
use crate::{
    git,
    ops::entry::{OperationKind, SnapshotDetails},
    virtual_branches::{branch::BranchUpdateRequest, Branch},
};

use super::entry::Trailer;

/// Snapshot functionality
impl Project {
    pub(crate) fn snapshot_branch_applied(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::ApplyBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    pub(crate) fn snapshot_branch_unapplied(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::UnapplyBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    pub(crate) fn snapshot_commit_undo(&self, commit_sha: git::Oid) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::UndoCommit).with_trailers(vec![Trailer {
                key: "sha".to_string(),
                value: commit_sha.to_string(),
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    pub(crate) fn snapshot_commit_creation(
        &self,
        snapshot_tree: git::Oid,
        commit_message: String,
        sha: Option<git::Oid>,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::CreateCommit).with_trailers(vec![
            Trailer {
                key: "message".to_string(),
                value: commit_message,
            },
            Trailer {
                key: "sha".to_string(),
                value: sha.map(|sha| sha.to_string()).unwrap_or_default(),
            },
        ]);
        self.commit_snapshot(snapshot_tree, details)?;
        Ok(())
    }
    pub(crate) fn snapshot_branch_creation(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::CreateBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    pub(crate) fn snapshot_branch_deletion(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationKind::DeleteBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name.to_string(),
            }]);

        self.create_snapshot(details)?;
        Ok(())
    }
    pub(crate) fn snapshot_branch_update(
        &self,
        old_branch: &Branch,
        update: &BranchUpdateRequest,
    ) -> anyhow::Result<()> {
        let details = if update.ownership.is_some() {
            SnapshotDetails::new(OperationKind::MoveHunk).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: old_branch.name.to_string(),
            }])
        } else if let Some(name) = update.name.as_deref() {
            SnapshotDetails::new(OperationKind::UpdateBranchName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch.name.clone(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: name.to_owned(),
                },
            ])
        } else if update.notes.is_some() {
            SnapshotDetails::new(OperationKind::UpdateBranchNotes)
        } else if let Some(order) = update.order {
            SnapshotDetails::new(OperationKind::ReorderBranches).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch.order.to_string(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: order.to_string(),
                },
            ])
        } else if let Some(_selected_for_changes) = update.selected_for_changes {
            SnapshotDetails::new(OperationKind::SelectDefaultVirtualBranch).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch
                        .selected_for_changes
                        .unwrap_or_default()
                        .to_string(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: old_branch.name.clone(),
                },
            ])
        } else if let Some(upstream) = update.upstream.as_deref() {
            SnapshotDetails::new(OperationKind::UpdateBranchRemoteName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch
                        .upstream
                        .as_ref()
                        .map(|r| r.to_string())
                        .unwrap_or_default(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: upstream.to_owned(),
                },
            ])
        } else {
            SnapshotDetails::new(OperationKind::GenericBranchUpdate)
        };
        self.create_snapshot(details)?;
        Ok(())
    }
}
