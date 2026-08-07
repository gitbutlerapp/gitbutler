//! Putting the landed commit onto the target: a local two-ref move for a self-remote, or a push
//! for a real remote. Both report a moved-target race as a retryable `Code::GitNonFastForward`.
//!
//! Lifted from the `but land` CLI command. The push path still takes `but_ctx::Context` because the
//! only push helper available today is the legacy `gitbutler_git` one; this is the one piece of land
//! that keeps a `Context` dependency until a graph-shaped push primitive exists.

use anyhow::bail;
use but_ctx::Context;
use gitbutler_git::GitContextExt as _;
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
    push_refspec(
        ctx,
        new_target_oid,
        push_remote_tracking_ref,
        refspec,
        false,
    )
}

/// Land's push convention for one explicit refspec: no push options, and forceless unless
/// `with_lease` asks for `--force-with-lease` against our remote-tracking ref (a compare-and-swap:
/// the push is rejected when the remote no longer matches what we last fetched).
/// `remote_tracking_ref` only names the remote to push to, and `head` is unused when an explicit
/// refspec is given. Enables the askpass broker (`Some(None)`: no stack context) so authenticated
/// remotes get credentials when called from desktop/SDK, where the broker is installed. The CLI
/// disables askpass (`but_askpass::disable()` in main.rs), so there it falls back to git's own
/// non-interactive credential helpers.
fn push_refspec(
    ctx: &Context,
    head: gix::ObjectId,
    remote_tracking_ref: String,
    refspec: String,
    with_lease: bool,
) -> anyhow::Result<()> {
    ctx.push(
        head,
        remote_tracking_ref,
        with_lease,
        with_lease,
        Some(refspec),
        Some(None),
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

/// Best-effort cleanup after a successful land: delete each landed branch's copy on the push
/// remote — the direct-push counterpart of the forge's "delete branch after merge". A stale pushed
/// copy is not just clutter: a branch later created with the same name is auto-associated with the
/// leftover remote-tracking ref and classified as merged upstream, refusing commits.
///
/// Only branches whose remote tip is contained in `target_tip` are deleted, so remote commits that
/// did not land are never discarded. Failures only warn — the land itself already succeeded.
/// Returns the short names of the branches whose remote copy was deleted.
pub(super) fn delete_landed_remote_branches(
    ctx: &Context,
    landed_branches: &[String],
    push_remote_name: &str,
    target_tip: gix::ObjectId,
    local_delivery: bool,
) -> Vec<String> {
    let mut deleted = Vec::new();
    for name in landed_branches {
        match delete_landed_remote_branch(ctx, name, push_remote_name, target_tip, local_delivery) {
            Ok(true) => deleted.push(name.clone()),
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(?err, branch = %name, "failed to delete the landed branch's remote copy");
            }
        }
    }
    deleted
}

/// Delete one landed branch's remote copy, returning whether it existed, was contained in the
/// landed target, and was deleted. On the self-remote path the "remote branch" is the local branch
/// itself (the reconcile removes it), so only the remote-tracking ref is deleted.
fn delete_landed_remote_branch(
    ctx: &Context,
    branch_name: &str,
    push_remote_name: &str,
    target_tip: gix::ObjectId,
    local_delivery: bool,
) -> anyhow::Result<bool> {
    let (tracking_ref_name, remote_branch_name) = {
        let repo = ctx.repo.get()?;
        let local_ref: gix::refs::FullName = format!("refs/heads/{branch_name}").try_into()?;
        // Resolve the same association the merged-upstream classification uses — the configured
        // upstream when set, otherwise a unique name match — rather than assuming
        // `<push remote>/<short name>`. A branch with no remote copy resolves to an error.
        let Ok(tracking_ref) =
            but_core::branch::resolve_tracking_branch_ref_name(local_ref.as_ref(), &repo)
        else {
            return Ok(false);
        };
        let Some((remote_name, remote_branch_name)) = but_core::extract_remote_name_and_short_name(
            tracking_ref.as_ref(),
            &repo.remote_names(),
        ) else {
            return Ok(false);
        };
        // Only the push remote is in scope; never push deletions to another remote. And only a
        // same-named remote branch counts as the landed branch's copy: a differently named
        // configured upstream is a fork point, not a copy — `git checkout -b topic origin/main`
        // makes `topic` track `origin/main`, and deleting that would delete the target itself.
        if remote_name != push_remote_name || remote_branch_name != branch_name {
            return Ok(false);
        }
        let tracking_ref_name = tracking_ref.as_ref().as_bstr().to_string();
        let Some(remote_tip) = super::peel_ref(&repo, &tracking_ref_name)? else {
            return Ok(false);
        };
        if !super::target_ref_contains(&repo, remote_tip, target_tip)? {
            return Ok(false);
        }
        (tracking_ref_name, remote_branch_name)
    };
    if !local_delivery {
        // Leased: the delete is rejected if the remote branch moved since our fetch, so commits
        // pushed there mid-land are never discarded on the strength of a stale containment check.
        push_refspec(
            ctx,
            target_tip,
            tracking_ref_name.clone(),
            format!(":refs/heads/{remote_branch_name}"),
            true,
        )?;
    }
    // Best-effort: the remote copy is gone either way, so a failed tracking-ref delete must not
    // make the deletion go unreported.
    let repo = ctx.repo.get()?;
    if let Ok(Some(reference)) = repo.try_find_reference(&tracking_ref_name)
        && let Err(err) = reference.delete()
    {
        tracing::warn!(?err, branch = %branch_name, "failed to delete the local remote-tracking ref");
    }
    Ok(true)
}
