use but_ctx::access::RepoExclusive;

use super::entry::Trailer;
use crate::{
    entry::{OperationKind, SnapshotDetails},
    oplog::OplogExt,
};

pub trait SnapshotExt {
    fn snapshot_stash_into_branch(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_branch_creation(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_create_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_remove_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_update_dependent_branch_name(
        &self,
        new_branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;
}

/// Snapshot functionality
impl SnapshotExt for but_ctx::Context {
    fn snapshot_stash_into_branch(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::StashIntoBranch)
            .with_trailers([Trailer::Name(branch_name)]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }

    fn snapshot_branch_creation(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(branch_name)]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }

    fn snapshot_create_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::CreateDependentBranch)
            .with_trailers([Trailer::Name(branch_name.to_owned())]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }

    fn snapshot_remove_dependent_branch(
        &self,
        branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::RemoveDependentBranch)
            .with_trailers([Trailer::Name(branch_name.to_owned())]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }

    fn snapshot_update_dependent_branch_name(
        &self,
        new_branch_name: &str,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::UpdateDependentBranchName)
            .with_trailers([Trailer::Name(new_branch_name.to_owned())]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }
}
