use crate::{
    ops::entry::{OperationType, SnapshotDetails},
    virtual_branches::Branch,
};

use super::{entry::Trailer, oplog::Oplog};

pub trait Snapshoter {
    fn snapshot_deletion(&self, oplog: &dyn Oplog) -> anyhow::Result<()>;
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
}
