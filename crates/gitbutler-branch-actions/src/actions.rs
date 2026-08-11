use anyhow::Result;
use but_ctx::{Context, access::RepoExclusive};
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::RemoteRefname;

use crate::{base, base::BaseBranch};

pub fn set_base_branch(
    ctx: &Context,
    target_branch: &RemoteRefname,
    perm: &mut RepoExclusive,
) -> Result<BaseBranch> {
    let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::SetBaseBranch), perm);
    base::set_base_branch(ctx, perm, target_branch)
}
