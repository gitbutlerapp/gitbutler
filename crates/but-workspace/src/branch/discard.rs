use but_core::{RefMetadata, WORKSPACE_REF_NAME, ref_metadata::WorkspaceCommitRelation};
use gix::refs::transaction::{Change, PreviousValue, RefEdit, RefLog};

/// Tear down the managed workspace so a fresh one can be built from the current `HEAD`.
///
/// Deletes the `gitbutler/workspace` reference (if present) and marks every stack it contained as
/// out-of-workspace in `meta`, leaving those stacks in metadata but unapplied. With the reference
/// gone, a subsequent [`apply`](crate::branch::apply) from an ad-hoc `HEAD` rebuilds the workspace
/// from scratch — the checked-out branch plus the one being applied — instead of reattaching to the
/// old workspace merge.
///
/// `repo` provides the reference to delete; `meta` is the workspace metadata to update.
///
/// The reference is deleted *before* the metadata is touched on purpose: metadata writes reconcile
/// against the live workspace, so the reference must be gone first or the out-of-workspace marks
/// would be reverted from the old merge commit.
pub fn discard_managed_workspace(
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
) -> anyhow::Result<()> {
    let ws_ref_name = gix::refs::FullName::try_from(WORKSPACE_REF_NAME)?;
    if repo.try_find_reference(ws_ref_name.as_ref())?.is_some() {
        repo.edit_reference(RefEdit {
            change: Change::Delete {
                log: RefLog::AndReference,
                expected: PreviousValue::Any,
            },
            name: ws_ref_name.clone(),
            deref: false,
        })?;
    }
    if let Some(mut ws_md) = meta.workspace_opt(ws_ref_name.as_ref())? {
        for stack in &mut ws_md.stacks {
            stack.workspacecommit_relation = WorkspaceCommitRelation::Outside;
        }
        meta.set_workspace(&ws_md)?;
    }
    Ok(())
}
