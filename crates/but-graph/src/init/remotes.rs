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
    // First try the standard lookup
    if let Some(tracking_ref) = lookup_remote_tracking_branch(repo, ref_name)? {
        return Ok(Some(tracking_ref));
    }

    // Legacy fallback using symbolic remote names
    for symbolic_remote_name in symbolic_remote_names {
        // Deduce the ref-name as fallback.
        // TODO: remove this - this is only required to support legacy repos that
        //       didn't setup normal Git remotes.
        let remote_tracking_ref_name = format!(
            "refs/remotes/{symbolic_remote_name}/{short_name}",
            short_name = ref_name.shorten()
        );
        let Ok(remote_tracking_ref_name) = gix::refs::FullName::try_from(remote_tracking_ref_name)
        else {
            continue;
        };
        if configured_remote_tracking_branches.contains(&remote_tracking_ref_name) {
            continue;
        }
        if let Ok(remote_ref) = repo.find_reference(remote_tracking_ref_name.as_ref()) {
            return Ok(Some(remote_ref.name().to_owned()));
        }
    }

    // GitButler unified fallback - use the same logic as push status calculation
    // Check common remotes for the remote tracking reference
    let branch_name = ref_name.shorten().to_string();
    for remote_name in crate::remote_ref_utils::COMMON_REMOTES {
        let remote_ref_name = format!("refs/remotes/{remote_name}/{branch_name}");

        if let Ok(remote_ref_fullname) = gix::refs::FullName::try_from(remote_ref_name.clone()) {
            if let Ok(Some(_)) = repo.try_find_reference(remote_ref_fullname.as_ref()) {
                tracing::debug!(
                    branch = branch_name,
                    remote = remote_name,
                    remote_ref = remote_ref_name,
                    "GitButler fallback: Found remote tracking reference"
                );
                return Ok(Some(remote_ref_fullname));
            }
        }
    }

    Ok(None)
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
    let branch_name = ref_name.shorten();

    // Handles refspec reverse-mapping - converts local branch refs to their corresponding
    // upstream remote refs using Git's configured remote tracking relationships
    if let Ok(Some((upstream_branch, _remote))) =
        repo.upstream_branch_and_remote_for_tracking_branch(ref_name)
    {
        return Ok(Some(upstream_branch));
    }

    // Second try with the existing repository
    match repo.branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch) {
        Some(Ok(result)) => {
            let owned_result = result.into_owned();
            tracing::debug!(
                branch = branch_name.to_string(),
                tracking_ref = owned_result.as_bstr().to_string(),
                "Found tracking ref using cached repository config"
            );
            return Ok(Some(owned_result));
        }
        Some(Err(err)) => {
            tracing::debug!(
                branch = branch_name.to_string(),
                error = %err,
                "Error reading tracking ref from cached repository config"
            );
        }
        None => {
            tracing::debug!(
                branch = branch_name.to_string(),
                "No tracking ref found in cached repository config"
            );
        }
    }

    // Final fallback: read Git config using gix's direct config API.
    // Required for accurate force push detection when gix's higher-level tracking resolution
    // fails. Uses gix config reading to get branch.*.remote values directly.
    let config_key = format!("branch.{branch_name}.remote");
    if let Some(remote_name_bstr) = repo.config_snapshot().string(&config_key) {
        let remote_name = remote_name_bstr.to_string();
        if !remote_name.is_empty() {
            // Construct the tracking ref from the configured remote
            let tracking_ref_name = format!("refs/remotes/{remote_name}/{branch_name}");
            if let Ok(tracking_ref) = gix::refs::FullName::try_from(tracking_ref_name) {
                tracing::debug!(
                    branch = branch_name.to_string(),
                    remote = remote_name,
                    tracking_ref = tracking_ref.as_bstr().to_string(),
                    "Found branch tracking configuration via gix config fallback"
                );
                return Ok(Some(tracking_ref));
            }
        }
    }

    Ok(None)
}
