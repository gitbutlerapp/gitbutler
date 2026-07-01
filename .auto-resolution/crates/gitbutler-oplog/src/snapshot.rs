use anyhow::Result;
use but_ctx::access::RepoExclusive;
use gitbutler_branch::BranchUpdateRequest;

use super::entry::Trailer;
use crate::{
    entry::{OperationKind, SnapshotDetails},
    oplog::OplogExt,
};

/// The name of a reference i.e. `refs/heads/master`
pub type ReferenceName = String;

pub trait SnapshotExt {
    fn snapshot_branch_unapplied(
        &self,
        snapshot_tree: gix::ObjectId,
        result: Result<&ReferenceName, &anyhow::Error>,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_commit_undo(
        &self,
        snapshot_tree: gix::ObjectId,
        result: Result<&(), &anyhow::Error>,
        commit_sha: gix::ObjectId,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_commit_creation(
        &self,
        snapshot_tree: gix::ObjectId,
        error: Option<&anyhow::Error>,
        commit_message: String,
        sha: Option<gix::ObjectId>,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

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

    fn snapshot_branch_deletion(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()>;

    fn snapshot_branch_update(
        &self,
        snapshot_tree: gix::ObjectId,
        old_order: usize,
        update: &BranchUpdateRequest,
        error: Option<&anyhow::Error>,
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
    fn snapshot_branch_unapplied(
        &self,
        snapshot_tree: gix::ObjectId,
        result: Result<&ReferenceName, &anyhow::Error>,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let result = result.map(|s| s.to_owned());
        let details =
            SnapshotDetails::new(OperationKind::UnapplyBranch).with_trailers(match result {
                Ok(name) => [Trailer::Name(name)],
                Err(err) => [Trailer::Error(err.to_string())],
            });
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }

    fn snapshot_commit_undo(
        &self,
        snapshot_tree: gix::ObjectId,
        result: Result<&(), &anyhow::Error>,
        commit_sha: gix::ObjectId,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::UndoCommit).with_trailers(match result {
            Ok(_) => [Trailer::Sha(commit_sha)],
            Err(err) => [Trailer::Error(err.to_string())],
        });
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }

    fn snapshot_commit_creation(
        &self,
        snapshot_tree: gix::ObjectId,
        error: Option<&anyhow::Error>,
        commit_message: String,
        sha: Option<gix::ObjectId>,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::CreateCommit).with_trailers(
            [Trailer::Message(commit_message)]
                .into_iter()
                .chain(sha.map(Trailer::Sha))
                .chain(error_trailer(error)),
        );
        self.commit_snapshot(snapshot_tree, details, perm)?;
        Ok(())
    }

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

    fn snapshot_branch_deletion(
        &self,
        branch_name: String,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = SnapshotDetails::new(OperationKind::DeleteBranch)
            .with_trailers([Trailer::Name(branch_name)]);
        self.create_snapshot(details, perm)?;
        Ok(())
    }

    fn snapshot_branch_update(
        &self,
        snapshot_tree: gix::ObjectId,
        old_order: usize,
        update: &BranchUpdateRequest,
        error: Option<&anyhow::Error>,
        perm: &mut RepoExclusive,
    ) -> anyhow::Result<()> {
        let details = if let Some(order) = update.order {
            SnapshotDetails::new(OperationKind::ReorderBranches).with_trailers(
                [Trailer::Before(old_order), Trailer::After(order)]
                    .into_iter()
                    .chain(error_trailer(error)),
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

fn error_trailer(error: Option<&anyhow::Error>) -> Option<Trailer> {
    error.map(|e| Trailer::Error(e.to_string()))
}
