use anyhow::Result;
use but_api_macros::but_api;
use but_cherry_apply::CherryApplyStatus;
use gitbutler_stack::StackId;
use tracing::instrument;

/// Return the cherry-pick applicability of `subject` using the behavior documented by
/// [`cherry_apply_status_with_perm()`].
///
/// This acquires shared worktree access from `ctx` before reading the
/// workspace state used for the applicability check.
#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply_status(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
) -> Result<CherryApplyStatus> {
    let guard = ctx.shared_worktree_access();
    cherry_apply_status_with_perm(ctx, subject, guard.read_permission())
}

/// Return the cherry-pick applicability of `subject` while reusing the shared repository
/// access in `perm`.
///
/// This reports whether `subject` can be applied in the current workspace state using
/// [`but_cherry_apply::cherry_apply_status()`].
pub fn cherry_apply_status_with_perm(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
    perm: &but_core::sync::RepoShared,
) -> Result<CherryApplyStatus> {
    but_cherry_apply::cherry_apply_status(ctx, perm, subject)
}

/// Cherry-pick `subject` into the stack identified by `target` using the behavior
/// documented by [`cherry_apply_with_perm()`].
///
/// This acquires exclusive worktree access from `ctx` before applying the commit.
#[but_api]
#[instrument(err(Debug))]
pub fn cherry_apply(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
    target: StackId,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    cherry_apply_with_perm(ctx, subject, target, guard.write_permission())
}

/// Cherry-pick `subject` into the stack identified by `target` while reusing the
/// exclusive repository access in `perm`.
///
/// The actual apply operation is performed by [`but_cherry_apply::cherry_apply()`].
pub fn cherry_apply_with_perm(
    ctx: &mut but_ctx::Context,
    subject: gix::ObjectId,
    target: StackId,
    perm: &mut but_core::sync::RepoExclusive,
) -> Result<()> {
    but_cherry_apply::cherry_apply(ctx, perm, subject, target)
}
