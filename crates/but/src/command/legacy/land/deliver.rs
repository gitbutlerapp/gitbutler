//! Putting the landed commit onto the target: a local two-ref move for a self-remote, or a push
//! for a real remote. Both report a moved-target race as a retryable `Code::GitNonFastForward`.

use anyhow::bail;
use but_ctx::Context;
use gitbutler_git::GitContextExt;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
};

/// Self-remote (`gb-local`) path: move `refs/heads/<target>` and the remote-tracking ref to the
/// landed commit in a single transaction, guarded by a compare-and-swap on the previous target.
pub(super) fn update_local_target_refs(
    repo: &gix::Repository,
    new_target_oid: gix::ObjectId,
    expected_target_oid: gix::ObjectId,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let head_ref = format!("refs/heads/{target_branch_name}");
    let tracking_ref = format!("refs/remotes/{push_remote_name}/{target_branch_name}");

    // The two refs must already point at the same commit. If local `<target>` has diverged from
    // `<remote>/<target>`, the shared compare-and-swap below can never succeed (the head edit keeps
    // failing), so surface that as a clear, non-retryable error rather than looping on it.
    if let (Some(head), Some(tracking)) = (
        super::peel_ref(repo, &head_ref)?,
        super::peel_ref(repo, &tracking_ref)?,
    ) && head != tracking
    {
        bail!(
            "Local `{target_branch_name}` ({head}) is out of sync with \
             `{push_remote_name}/{target_branch_name}` ({tracking}). Run `but pull` to resync first."
        );
    }

    // Advance both refs to the landed commit in one transaction, failing if either moved meanwhile.
    let advance = Change::Update {
        log: LogChange {
            mode: RefLog::AndReference,
            force_create_reflog: false,
            message: "GitButler land".into(),
        },
        expected: PreviousValue::ExistingMustMatch(expected_target_oid.into()),
        new: Target::Object(new_target_oid),
    };
    let edits = [
        RefEdit {
            change: advance.clone(),
            name: head_ref.try_into()?,
            deref: false,
        },
        RefEdit {
            change: advance,
            name: tracking_ref.try_into()?,
            deref: false,
        },
    ];

    match repo.edit_references(edits) {
        Ok(_) => Ok(()),
        // A concurrent move broke the compare-and-swap — the same retryable signal as a
        // non-fast-forward push, so the retry loop re-fetches and tries again.
        Err(err) if is_ref_out_of_date(&err) => {
            Err(anyhow::Error::new(err).context(but_error::Code::GitNonFastForward))
        }
        Err(err) => Err(err.into()),
    }
}

/// Whether a `repo.edit_references` failure was a compare-and-swap mismatch (the expected previous
/// value didn't match), i.e. a ref moved concurrently — as opposed to an I/O or lock failure.
fn is_ref_out_of_date(err: &gix::reference::edit::Error) -> bool {
    matches!(
        err,
        gix::reference::edit::Error::FileTransactionPrepare(
            gix::refs::file::transaction::prepare::Error::ReferenceOutOfDate { .. }
        )
    )
}

/// Real-remote path: push the landed commit onto the target branch. Fast-forward by default
/// (no force); a non-fast-forward rejection surfaces as `Code::GitNonFastForward` for the retry.
pub(super) fn push_to_target(
    ctx: &Context,
    new_target_oid: gix::ObjectId,
    push_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let push_remote_tracking_ref = format!("refs/remotes/{push_remote_name}/{target_branch_name}");
    let refspec = format!("{new_target_oid}:refs/heads/{target_branch_name}");
    // Askpass is disabled in the CLI (`but_askpass::disable()` in main.rs), so authenticated
    // remotes rely on git's own non-interactive credential helpers; passing a broker is inert.
    ctx.push(
        new_target_oid,
        push_remote_tracking_ref,
        false,
        false,
        Some(refspec),
        None,
        vec![],
    )?;
    Ok(())
}

/// Both the non-fast-forward push rejection and the self-remote compare-and-swap failure are tagged
/// with `Code::GitNonFastForward`; either means the target moved and we should re-fetch and retry.
pub(super) fn is_retryable_concurrency_error(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<but_error::Code>(),
        Some(but_error::Code::GitNonFastForward)
    )
}
