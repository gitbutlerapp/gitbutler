use std::collections::{BTreeMap, BTreeSet};

use bstr::BString;
use gix::refs::Category;

use crate::init::overlay::OverlayRepo;

/// Return the configured fetch-side upstream for `ref_name`.
pub fn lookup_remote_tracking_branch(
    repo: &OverlayRepo<'_>,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<gix::refs::FullName>> {
    Ok(repo
        .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
        .transpose()?
        .map(|name| name.into_owned()))
}

/// Resolve each visible local branch's effective fetch-side remote-tracking ref.
///
/// Git configuration is authoritative, even when its upstream ref is currently absent. A local
/// branch without a configured upstream may use the same short name on a configured remote only
/// when exactly one such ref exists. Configured upstream names are reserved from this fallback so
/// one remote-tracking ref cannot be inferred for a different local branch.
pub fn effective_remote_tracking_branches(
    repo: &OverlayRepo<'_>,
) -> anyhow::Result<BTreeMap<gix::refs::FullName, gix::refs::FullName>> {
    let local_refs = repo.collect_ref_mapping_by_prefix(["refs/heads/"].into_iter(), &[])?;
    let local_names = local_refs
        .into_values()
        .flatten()
        .filter(|name| name.category() == Some(Category::LocalBranch))
        .collect::<BTreeSet<_>>();

    let mut configured = BTreeMap::new();
    let mut configured_claims = BTreeSet::new();
    for local_name in &local_names {
        if let Some(remote_name) = lookup_remote_tracking_branch(repo, local_name.as_ref())? {
            configured_claims.insert(remote_name.clone());
            configured.insert(local_name.clone(), remote_name);
        }
    }

    let remote_names = repo.remote_names();
    for local_name in local_names {
        if configured.contains_key(&local_name) {
            continue;
        }
        let mut candidates = Vec::new();
        for remote_name in &remote_names {
            let mut candidate = BString::from("refs/remotes/");
            candidate.extend_from_slice(remote_name.as_ref());
            candidate.push(b'/');
            candidate.extend_from_slice(local_name.shorten());
            let candidate: gix::refs::FullName = candidate.try_into()?;
            if configured_claims.contains(&candidate) {
                continue;
            }
            if repo.try_find_reference(candidate.as_ref())?.is_some() {
                candidates.push(candidate);
            }
        }
        if let [candidate] = candidates.as_slice() {
            configured.insert(local_name, candidate.clone());
        }
    }
    Ok(configured)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use but_testsupport::InMemoryRefMetadata;

    use super::*;
    use crate::init::Overlay;

    fn name(value: &str) -> Result<gix::refs::FullName> {
        Ok(value.try_into()?)
    }

    fn with_repo(f: impl FnOnce(&OverlayRepo<'_>) -> Result<()>) -> Result<()> {
        let root = but_testsupport::gix_testtools::scripted_fixture_read_only("scenarios.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let repo = gix::open_opts(
            root.join("effective-upstream-rules"),
            gix::open::Options::isolated(),
        )?
        .with_object_memory();
        let meta = InMemoryRefMetadata::default();
        let (repo, _meta, _entrypoint) = Overlay::default().into_parts(&repo, &meta);
        f(&repo)
    }

    #[test]
    fn configured_upstream_wins_over_same_name_fallback() -> Result<()> {
        with_repo(|repo| {
            let actual = effective_remote_tracking_branches(repo)?;
            assert_eq!(
                actual.get(&name("refs/heads/configured")?),
                Some(&name("refs/remotes/origin/special")?)
            );
            assert_eq!(
                actual.get(&name("refs/heads/configured-missing")?),
                Some(&name("refs/remotes/origin/missing")?),
                "configured upstream identity survives an absent ref"
            );
            Ok(())
        })
    }

    #[test]
    fn unique_same_name_remote_is_the_fallback() -> Result<()> {
        with_repo(|repo| {
            let actual = effective_remote_tracking_branches(repo)?;
            assert_eq!(
                actual.get(&name("refs/heads/unique")?),
                Some(&name("refs/remotes/origin/unique")?)
            );
            Ok(())
        })
    }

    #[test]
    fn ambiguous_same_name_remotes_have_no_fallback() -> Result<()> {
        with_repo(|repo| {
            let actual = effective_remote_tracking_branches(repo)?;
            assert!(
                !actual.contains_key(&name("refs/heads/ambiguous")?),
                "multiple existing candidates are ambiguous"
            );
            Ok(())
        })
    }

    #[test]
    fn configured_claim_reserves_remote_from_fallback() -> Result<()> {
        with_repo(|repo| {
            let actual = effective_remote_tracking_branches(repo)?;
            assert_eq!(
                actual.get(&name("refs/heads/claimant")?),
                Some(&name("refs/remotes/origin/reserved")?)
            );
            assert!(
                !actual.contains_key(&name("refs/heads/reserved")?),
                "a configured claim cannot be inferred for another local"
            );
            Ok(())
        })
    }
}
