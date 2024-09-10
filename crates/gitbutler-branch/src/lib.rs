mod branch;

use anyhow::Context;
pub use branch::{Branch, BranchCreateRequest, BranchId, BranchIdentity, BranchUpdateRequest};
use bstr::ByteSlice;
mod branch_ext;
pub use branch_ext::BranchExt;
mod reference_ext;
pub use reference_ext::{ReferenceExt, ReferenceExtGix};
mod dedup;
pub use dedup::{dedup, dedup_fmt};
mod file_ownership;
pub use file_ownership::OwnershipClaim;
mod ownership;
pub use ownership::{reconcile_claims, BranchOwnershipClaims, ClaimOutcome};
pub mod serde;
mod target;
pub use target::Target;
mod reference;
pub use reference::ChangeReference;

mod state;
use lazy_static::lazy_static;
pub use state::{VirtualBranches as VirtualBranchesState, VirtualBranchesHandle};
lazy_static! {
    pub static ref GITBUTLER_WORKSPACE_REFERENCE: gitbutler_reference::LocalRefname =
        gitbutler_reference::LocalRefname::new("gitbutler/workspace", None);
}

pub const GITBUTLER_COMMIT_AUTHOR_NAME: &str = "GitButler";
pub const GITBUTLER_COMMIT_AUTHOR_EMAIL: &str = "gitbutler@gitbutler.com";

pub enum SignaturePurpose {
    Author,
    Committer,
}

/// Provide a signature with the GitButler author, and the current time or the time overridden
/// depending on the value for `purpose`.
pub fn signature(purpose: SignaturePurpose) -> anyhow::Result<git2::Signature<'static>> {
    let signature = gix::actor::SignatureRef {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: commit_time(match purpose {
            SignaturePurpose::Author => "GIT_AUTHOR_DATE",
            SignaturePurpose::Committer => "GIT_COMMITTER_DATE",
        }),
    };
    gix_to_git2_signature(signature)
}

/// Convert `actor` to a `git2` representation or fail if that's not possible.
/// Note that the current time as provided by `gix` is also used as it.
pub fn gix_to_git2_signature(
    actor: gix::actor::SignatureRef<'_>,
) -> anyhow::Result<git2::Signature<'static>> {
    let offset_in_minutes = actor.time.offset / 60;
    let time = git2::Time::new(actor.time.seconds, offset_in_minutes);
    Ok(git2::Signature::new(
        actor
            .name
            .to_str()
            .with_context(|| format!("Could not process actor name: {}", actor.name))?,
        actor
            .email
            .to_str()
            .with_context(|| format!("Could not process actor email: {}", actor.email))?,
        &time,
    )?)
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}
