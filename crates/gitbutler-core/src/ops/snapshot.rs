use std::vec;

use crate::{
    ops::entry::{OperationType, SnapshotDetails},
    virtual_branches::{branch::BranchUpdateRequest, Branch},
};

use super::{entry::Trailer, oplog::Oplog};

pub trait Snapshoter {
    fn snapshot_deletion(&self, oplog: &dyn Oplog) -> anyhow::Result<()>;
    fn snapshot_update(&self, oplog: &dyn Oplog, update: BranchUpdateRequest)
        -> anyhow::Result<()>;
}

impl Snapshoter for Branch {
    fn snapshot_deletion(&self, oplog: &dyn Oplog) -> anyhow::Result<()> {
        let details =
            SnapshotDetails::new(OperationType::DeleteBranch).with_trailers(vec![Trailer {
                key: "name".to_string(),
                value: self.name.to_string(),
            }]);

        oplog.create_snapshot(details)?;
        Ok(())
    }
    fn snapshot_update(
        &self,
        oplog: &dyn Oplog,
        update: BranchUpdateRequest,
    ) -> anyhow::Result<()> {
        let details = if update.ownership.is_some() {
            SnapshotDetails::new(OperationType::MoveHunk)
        } else if let Some(name) = update.name {
            SnapshotDetails::new(OperationType::UpdateBranchName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: self.name.to_string(),
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
                    value: self.order.to_string(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: order.to_string(),
                },
            ])
        } else if let Some(selected_for_changes) = update.selected_for_changes {
            SnapshotDetails::new(OperationType::SelectDefaultVirtualBranch).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: self.selected_for_changes.unwrap_or_default().to_string(),
                },
                Trailer {
                    key: "after".to_string(),
                    value: selected_for_changes.to_string(),
                },
            ])
        } else if let Some(upstream) = update.upstream {
            SnapshotDetails::new(OperationType::UpdateBranchRemoteName).with_trailers(vec![
                Trailer {
                    key: "before".to_string(),
                    value: self
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
        oplog.create_snapshot(details)?;
        Ok(())
    }
}
