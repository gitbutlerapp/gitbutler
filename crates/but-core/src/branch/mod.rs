use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use anyhow::{Result, bail};
use bstr::{BStr, ByteSlice as _};
use gix::remote::Direction;

type WorktreePathByRef = BTreeMap<gix::refs::FullName, Vec<PathBuf>>;

mod normalize;
pub use normalize::normalize_short_name;

mod generate;
pub use generate::{
    canned_refname, find_unique_refname, find_unique_refname_excluding, refuse_colliding_refname,
    unique_canned_refname,
};

/// Return `true` if `identity` is a GitButler-internal branch name that must not
/// be shown or treated as a user branch.
pub fn is_technical_branch_name(identity: &BStr) -> bool {
    const TECHNICAL_IDENTITIES: &[&[u8]] = &[
        b"HEAD",
        b"gitbutler/edit",
        b"gitbutler/integration",
        b"gitbutler/oplog",
        b"gitbutler/target",
        b"gitbutler/workspace",
    ];
    debug_assert!(
        TECHNICAL_IDENTITIES.is_sorted(),
        "binary search requires sorted technical identities"
    );
    TECHNICAL_IDENTITIES
        .binary_search(&identity.as_bytes())
        .is_ok()
        || identity.starts_with(b"gitbutler/rename-backup/")
}

/// Resolve the remote-tracking ref that corresponds to a local branch ref.
///
/// `repo` provides branch configuration and ref lookup access used to resolve
/// the configured tracking branch or a unique fallback under `refs/remotes/*`.
pub fn resolve_tracking_branch_ref_name<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    repo: &'a gix::Repository,
) -> Result<Cow<'a, gix::refs::FullNameRef>> {
    if let Some(upstream_ref_name) = repo
        .branch_remote_tracking_ref_name(ref_name, Direction::Fetch)
        .transpose()?
        && repo
            .try_find_reference(upstream_ref_name.as_ref())?
            .is_some()
    {
        return Ok(Cow::Owned(upstream_ref_name));
    }

    let branch_name = ref_name.shorten();
    let mut remote_matches = repo
        .remote_names()
        .iter()
        .filter_map(|remote_name| {
            let full_name = format!("refs/remotes/{remote_name}/{branch_name}");
            repo.try_find_reference(&full_name)
                .transpose()
                .map(|reference| {
                    reference.map(|_| {
                        full_name
                            .try_into()
                            .expect("constructed remote-tracking refname must be valid")
                    })
                })
        })
        .collect::<Result<Vec<gix::refs::FullName>, _>>()?;

    if remote_matches.len() == 1 {
        return Ok(Cow::Owned(
            remote_matches
                .pop()
                .expect("exactly one remote match exists"),
        ));
    }

    bail!("Branch '{ref_name}' has no tracking branch")
}

/// A way to safely delete branches, which is only the case it's checked out nowhere.
pub mod safe_delete;

/// State for reuse when [safely deleting references](SafeDelete::delete_reference).
#[derive(Debug)]
pub struct SafeDelete {
    /// A mapping of one or more worktree paths that are affected by changes to the keyed reference name.
    worktrees_by_ref: WorktreePathByRef,
}
