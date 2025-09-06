use crate::init::overlay::OverlayRepo;
use gix::reference::Category;
use std::collections::BTreeSet;

/// Returns the unique names of all remote tracking branches that are configured in the repository.
/// Useful to avoid claiming them for deduction.
pub fn configured_remote_tracking_branches(
    repo: &OverlayRepo<'_>,
) -> anyhow::Result<BTreeSet<gix::refs::FullName>> {
    let mut out = BTreeSet::default();
    for short_name in repo
        .config_snapshot()
        .sections_by_name("branch")
        .into_iter()
        .flatten()
        .filter_map(|s| s.header().subsection_name())
    {
        let Ok(full_name) = Category::LocalBranch.to_full_name(short_name) else {
            continue;
        };
        out.extend(lookup_remote_tracking_branch(repo, full_name.as_ref())?);
    }
    Ok(out)
}

// Note that despite having multiple candidates for remote names, there can only be one
// remote per branch.
// TODO: remove deduction entirely by properly setting up remotes.
pub fn lookup_remote_tracking_branch_or_deduce_it(
    repo: &OverlayRepo<'_>,
    ref_name: &gix::refs::FullNameRef,
    symbolic_remote_names: &[String],
    configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
) -> anyhow::Result<Option<gix::refs::FullName>> {
    Ok(lookup_remote_tracking_branch(repo, ref_name)?.or_else(|| {
        for symbolic_remote_name in symbolic_remote_names {
            // Deduce the ref-name as fallback.
            // TODO: remove this - this is only required to support legacy repos that
            //       didn't setup normal Git remotes.
            let remote_tracking_ref_name = format!(
                "refs/remotes/{symbolic_remote_name}/{short_name}",
                short_name = ref_name.shorten()
            );
            let Ok(remote_tracking_ref_name) =
                gix::refs::FullName::try_from(remote_tracking_ref_name)
            else {
                continue;
            };
            if configured_remote_tracking_branches.contains(&remote_tracking_ref_name) {
                continue;
            }
            return repo
                .find_reference(remote_tracking_ref_name.as_ref())
                .ok()
                .map(|remote_ref| remote_ref.name().to_owned());
        }
        None
    }))
}

pub fn extract_remote_name(
    ref_name: &gix::refs::FullNameRef,
    remotes: &gix::remote::Names<'_>,
) -> Option<String> {
    let (category, shorthand_name) = ref_name.category_and_short_name()?;
    if !matches!(category, Category::RemoteBranch) {
        return None;
    }

    let longest_remote = remotes
        .iter()
        .rfind(|reference_name| shorthand_name.starts_with(reference_name))
        .ok_or(anyhow::anyhow!(
            "Failed to find remote branch's corresponding remote"
        ))
        .ok()?;
    Some(longest_remote.to_string())
}

pub fn lookup_remote_tracking_branch(
    repo: &OverlayRepo<'_>,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<gix::refs::FullName>> {
    Ok(repo
        .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
        .transpose()?
        .map(|rn| rn.into_owned()))
}
