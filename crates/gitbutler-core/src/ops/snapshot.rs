use std::vec;

use crate::{
    ops::entry::{OperationType, SnapshotDetails},
    virtual_branches::{branch::BranchUpdateRequest, Branch},
};

use super::{entry::Trailer, oplog::Oplog};

pub trait Snapshot {
    fn snapshot_branch_creation(&self, branch_name: String) -> anyhow::Result<()>;
    fn snapshot_branch_deletion(&self, branch_name: String) -> anyhow::Result<()>;
    fn snapshot_branch_applied(&self, branch_name: String) -> anyhow::Result<()>;
    fn snapshot_branch_unapplied(&self, branch_name: String) -> anyhow::Result<()>;
    fn snapshot_branch_update(
        &self,
        old_branch: &Branch,
        update: &BranchUpdateRequest,
    ) -> anyhow::Result<()>;
    fn snapshot_commit_creation(
        &self,
        commit_message: String,
        sha: Option<String>,
    ) -> anyhow::Result<()>;
    fn snapshot_commit_undo(&self, commit_sha: String) -> anyhow::Result<()>;
}

impl<T: Oplog> Snapshot for T {
    fn snapshot_branch_applied(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::ApplyBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_branch_unapplied(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::UnapplyBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_commit_undo(&self, commit_sha: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::UndoCommit).with_trailers(vec![Trailer {
                key: "sha".to_string(),
                value: commit_sha,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_commit_creation(
        &self,
        commit_message: String,
        sha: Option<String>,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationType::CreateCommit).with_trailers(vec![
            Trailer {
                key: "message".to_string(),
                value: commit_message,
            },
            Trailer {
                key: "sha".to_string(),
                value: sha.unwrap_or_default(),
            },
        ]);
        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_branch_creation(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::CreateBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name,
            }]);
        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_branch_deletion(&self, branch_name: String) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::DeleteBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: branch_name.to_string(),
            }]);

        self.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_branch_update(
        &self,
        old_branch: &Branch,
        update: &BranchUpdateRequest,
    ) -> anyhow::Result<()> {
        let details = if update.ownership.is_some() {
            SnapshotDetails::new(OperationType::MoveHunk).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: old_branch.name.to_string(),
            }])
        } else if let Some(name) = update.name.clone() {
            SnapshotDetails::new(OperationType::UpdateBranchName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch.name.to_string(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: name,
                },
            ])
        } else if update.notes.is_some() {
            SnapshotDetails::new(OperationType::UpdateBranchNotes)
        } else if let Some(order) = update.order {
            SnapshotDetails::new(OperationType::ReorderBranches).with_trailers(vec![
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
            SnapshotDetails::new(OperationType::SelectDefaultVirtualBranch).with_trailers(vec![
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
        } else if let Some(upstream) = update.upstream.clone() {
            SnapshotDetails::new(OperationType::UpdateBranchRemoteName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: old_branch
                        .upstream
                        .clone()
                        .map(|r| r.to_string())
                        .unwrap_or("".to_string()),
                },
                Trailer {
                    key: "after".to_string(),
                    value: upstream,
                },
            ])
        } else {
            SnapshotDetails::new(OperationType::GenericBranchUpdate)
        };
        self.create_snapshot(details)?;
        Ok(())
    }
}
