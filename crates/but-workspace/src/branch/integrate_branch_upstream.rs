use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result};
use but_core::RefMetadata;
use gix::{prelude::ObjectIdExt as _, remote::Direction};
use uuid::Uuid;

#[allow(missing_docs)]
#[derive(Debug)]
pub enum InteractiveIntegrationStep {
    Skip {
        id: Uuid,
        commit_id: gix::ObjectId,
    },
    Pick {
        id: Uuid,
        commit_id: gix::ObjectId,
    },
    PickUpstream {
        id: Uuid,
        commit_id: gix::ObjectId,
        upstream_commit_id: gix::ObjectId,
    },
    Squash {
        id: Uuid,
        commits: Vec<gix::ObjectId>,
        message: Option<String>,
    },
}

/// Get the initial integration steps for a branch.
///
/// The returned steps are ordered for application from parent to child so they
/// can be passed directly to integration without reordering by the caller.
pub fn get_initial_integration_steps_for_branch(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
    _meta: &mut impl RefMetadata,
) -> Result<Vec<InteractiveIntegrationStep>> {
    let (local_commits, upstream_commits) = get_commits_until_merge_base(ref_name, repo)?;

    let upstream_by_id = upstream_commits.iter().copied().collect::<HashSet<_>>();
    let mut upstream_by_change_id = HashMap::<String, gix::ObjectId>::new();
    for commit_id in &upstream_commits {
        let change_id = effective_change_id(repo, *commit_id)?;
        // Keep the first seen (closest to tip) upstream commit for stable matching.
        upstream_by_change_id.entry(change_id).or_insert(*commit_id);
    }

    let mut matched_upstream = HashSet::new();
    let mut local_result_order_commits = Vec::new();
    let mut divergence_local_only = Vec::new();
    let mut divergence_matched = Vec::new();
    for commit_id in local_commits {
        if upstream_by_id.contains(&commit_id) {
            matched_upstream.insert(commit_id);
            local_result_order_commits.push(commit_id);
            divergence_matched.push(divergence_commit(repo, commit_id)?);
            continue;
        }

        let change_id = effective_change_id(repo, commit_id)?;
        if let Some(upstream_commit_id) = upstream_by_change_id.get(&change_id) {
            matched_upstream.insert(*upstream_commit_id);
            local_result_order_commits.push(commit_id);
            divergence_matched.push(divergence_commit(repo, commit_id)?);
        } else {
            local_result_order_commits.push(commit_id);
            divergence_local_only.push(divergence_commit(repo, commit_id)?);
        }
    }

    let remote_only_commits = upstream_commits
        .into_iter()
        .filter(|id| !matched_upstream.contains(id));
    let mut divergence_upstream_only = Vec::new();

    let mut initial_steps = Vec::new();

    // Build the branch in natural tip-to-base result order first, then reverse
    // the whole sequence so the returned steps are ready to apply from the
    // merge-base upward.
    for commit in local_result_order_commits {
        initial_steps.push(InteractiveIntegrationStep::Pick {
            id: Uuid::new_v4(),
            commit_id: commit,
        });
    }

    for upstream_commit in remote_only_commits {
        initial_steps.push(InteractiveIntegrationStep::Pick {
            id: Uuid::new_v4(),
            commit_id: upstream_commit,
        });
    }

    for commit in local_and_remote_commits {
        initial_steps.push(InteractiveIntegrationStep::Pick {
            id: Uuid::new_v4(),
            commit_id: commit,
        });
    }

    Ok(initial_steps)
}

fn get_commits_until_merge_base(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
) -> Result<(Vec<gix::ObjectId>, Vec<gix::ObjectId>), anyhow::Error> {
    let (local_tip, upstream_ref_name, upstream_tip) =
        get_branch_tips_and_upstream(ref_name, repo)?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let merge_base = repo
        .merge_base_with_graph(local_tip.attach(repo), upstream_tip.attach(repo), &mut graph)
        .map(|id| id.detach())
        .map_err(|_| {
            anyhow::anyhow!(
                "No merge-base found between '{ref_name}' and its tracking branch '{upstream_ref_name}'"
            )
        })?;
    let local_commits = branch_commits_until(repo, local_tip, merge_base)?;
    let upstream_commits = branch_commits_until(repo, upstream_tip, merge_base)?;
    Ok((local_commits, upstream_commits))
}

fn get_branch_tips_and_upstream<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    repo: &'a gix::Repository,
) -> Result<
    (
        gix::ObjectId,
        std::borrow::Cow<'a, gix::refs::FullNameRef>,
        gix::ObjectId,
    ),
    anyhow::Error,
> {
    let mut local_branch = repo
        .find_reference(ref_name)
        .with_context(|| format!("Couldn't find local branch '{ref_name}'"))?;
    let local_tip = local_branch.peel_to_id()?.detach();
    let upstream_ref_name = resolve_tracking_branch_ref_name(ref_name, repo)?;
    let mut upstream_branch = repo
        .find_reference(upstream_ref_name.as_ref())
        .with_context(|| {
            format!(
                "Couldn't find tracking branch '{upstream_ref_name}' for local branch '{ref_name}'"
            )
        })?;
    let upstream_tip = upstream_branch.peel_to_id()?.detach();
    Ok((local_tip, upstream_ref_name, upstream_tip))
}

fn branch_commits_until(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    merge_base: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    let traversal = tip
        .attach(repo)
        .ancestors()
        .with_hidden(Some(merge_base))
        .first_parent_only()
        .all()?;

    let mut out = Vec::new();
    for info in traversal {
        out.push(info?.id);
    }
    Ok(out)
}

fn effective_change_id(repo: &gix::Repository, commit_id: gix::ObjectId) -> Result<String> {
    Ok(but_core::Commit::from_id(commit_id.attach(repo))?
        .change_id()
        .to_string())
}
