//! Read-only repository information.

use anyhow::Result;
use but_api_macros::but_api;
use but_ctx::Context;
use tracing::instrument;

/// Identify the repository by the oldest parentless commit reachable from HEAD.
///
/// Uses committer time, breaking ties by object ID. Returns `None` for unborn
/// HEAD or incomplete shallow ancestry, where the oldest root cannot be known.
/// Object lookup and decoding errors are propagated.
#[but_api(napi, provides = [RepositoryRootCommit])]
#[instrument(err(Debug))]
pub fn repository_root_commit(ctx: &Context) -> Result<Option<String>> {
    let guard = ctx.shared_worktree_access();
    Ok(repository_root_commit_with_perm(ctx, guard.read_permission())?.map(|id| id.to_string()))
}

/// Identify the repository while reusing the caller's repository permission.
pub fn repository_root_commit_with_perm(
    ctx: &Context,
    _perm: &but_core::sync::RepoShared,
) -> Result<Option<gix::ObjectId>> {
    let repo = ctx.repo.get()?;
    let mut head = repo.head()?;
    if head.is_unborn() {
        return Ok(None);
    }
    let head = head.peel_to_commit()?;
    let shallow = repo.shallow_commits()?;
    let mut oldest = None;
    let mut incomplete = false;

    // This identity needs complete Git ancestry, not the workspace graph's
    // bounded view. Inspect actual objects at candidate roots and shallow
    // boundaries so a grafted boundary can never masquerade as a root.
    for info in head.id().ancestors().all()? {
        let info = info?;
        let is_shallow = shallow
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&info.id).is_ok());
        if info.parent_ids.is_empty() || is_shallow {
            let commit = repo.find_commit(info.id)?;
            let decoded = commit.decode()?;
            if decoded.parents.is_empty() {
                let candidate = (decoded.time()?.seconds, info.id);
                oldest =
                    Some(oldest.map_or(candidate, |previous| std::cmp::min(previous, candidate)));
            } else if is_shallow {
                incomplete = true;
            }
        }
    }

    Ok(if incomplete {
        None
    } else {
        oldest.map(|(_, id)| id)
    })
}
