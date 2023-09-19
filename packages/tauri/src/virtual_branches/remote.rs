use std::{
    collections::{HashMap, HashSet},
    time,
};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    gb_repository, git,
    project_repository::{self, LogUntil},
    reader, sessions,
};

use super::{branch, get_default_target, iterator::BranchIterator as Iterator, Author};

// this struct is a mapping to the view `RemoteBranch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
//
// it holds data calculated for presentation purposes of one Git branch
// with comparison data to the Target commit, determining if it is mergeable,
// and how far ahead or behind the Target it is.
// an array of them can be requested from the frontend to show in the sidebar
// Tray and should only contain branches that have not been converted into
// virtual branches yet (ie, we have no `Branch` struct persisted in our data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranch {
    pub sha: String,
    pub name: String,
    pub behind: u32,
    pub upstream: Option<git::RemoteBranchName>,
    pub commits: Vec<RemoteCommit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCommit {
    pub id: String,
    pub description: String,
    pub created_at: u128,
    pub author: Author,
}

pub fn list_remote_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Vec<RemoteBranch>> {
    // get the current target
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(vec![]),
    };

    let current_time = time::SystemTime::now();
    let too_old = time::Duration::from_secs(86_400 * 90); // 90 days (3 months) is too old

    let repo = &project_repository.git_repository;

    let main_oid = default_target.sha;

    let virtual_branches_names = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter_map(|branch| branch.upstream)
        .map(|upstream| upstream.branch().to_string())
        .collect::<HashSet<_>>();
    let mut most_recent_branches_by_hash: HashMap<git::Oid, (git::Branch, u64)> = HashMap::new();

    for (branch, _) in repo.branches(None)?.flatten() {
        if let Some(branch_oid) = branch.target() {
            // get the branch ref
            let branch_commit = repo
                .find_commit(branch_oid)
                .context("failed to find branch commit")?;
            let branch_time = branch_commit.time();
            let seconds = branch_time
                .seconds()
                .try_into()
                .context("failed to convert seconds")?;
            let branch_time = time::UNIX_EPOCH + time::Duration::from_secs(seconds);
            let duration = current_time
                .duration_since(branch_time)
                .context("failed to get duration")?;
            if duration > too_old {
                continue;
            }

            let branch_name =
                git::BranchName::try_from(&branch).context("could not get branch name")?;

            // skip the default target branch (both local and remote)
            match branch_name {
                git::BranchName::Remote(ref remote_branch_name) => {
                    if *remote_branch_name == default_target.branch {
                        continue;
                    }
                }
                git::BranchName::Local(ref local_branch_name) => {
                    if let Some(upstream_branch_name) = local_branch_name.remote() {
                        if *upstream_branch_name == default_target.branch {
                            continue;
                        }
                    }
                }
            }

            if virtual_branches_names.contains(branch_name.branch()) {
                continue;
            }
            if branch_name.branch().eq("HEAD") {
                continue;
            }
            if branch_name
                .branch()
                .eq(super::integration::GITBUTLER_INTEGRATION_BRANCH_NAME)
            {
                continue;
            }

            match most_recent_branches_by_hash.get(&branch_oid) {
                Some((_, existing_seconds)) => {
                    let branch_name = branch.refname().context("could not get branch name")?;
                    if seconds < *existing_seconds {
                        // this branch is older than the one we already have
                        continue;
                    }
                    if seconds > *existing_seconds {
                        most_recent_branches_by_hash.insert(branch_oid, (branch, seconds));
                        continue;
                    }
                    if branch_name.starts_with("refs/remotes") {
                        // this branch is a remote branch
                        // we always prefer the remote branch if it is the same age as the local branch
                        most_recent_branches_by_hash.insert(branch_oid, (branch, seconds));
                        continue;
                    }
                }
                None => {
                    // this is the first time we've seen this branch
                    // so we should add it to the list
                    most_recent_branches_by_hash.insert(branch_oid, (branch, seconds));
                }
            }
        }
    }

    let mut most_recent_branches: Vec<(git::Branch, u64)> =
        most_recent_branches_by_hash.into_values().collect();

    // take the most recent 20 branches
    most_recent_branches.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by timestamp in descending order.
    let sorted_branches: Vec<git::Branch> = most_recent_branches
        .into_iter()
        .map(|(branch, _)| branch)
        .collect();
    let top_branches = sorted_branches.into_iter().take(20).collect::<Vec<_>>(); // Take the first 20 entries.

    let mut branches: Vec<RemoteBranch> = Vec::new();
    for branch in &top_branches {
        if let Some(branch_oid) = branch.target() {
            let ahead = project_repository
                .log(branch_oid, LogUntil::Commit(main_oid))
                .context("failed to get ahead commits")?;

            if ahead.is_empty() {
                continue;
            }

            let branch_name = branch.refname().context("could not get branch name")?;

            let count_behind = project_repository
                .distance(main_oid, branch_oid)
                .context("failed to get behind count")?;

            let upstream = branch
                .upstream()
                .ok()
                .map(|upstream_branch| git::RemoteBranchName::try_from(&upstream_branch))
                .transpose()?;

            branches.push(RemoteBranch {
                sha: branch_oid.to_string(),
                name: branch_name.to_string(),
                upstream,
                behind: count_behind,
                commits: ahead
                    .into_iter()
                    .map(|commit| commit_to_remote_commit(&commit))
                    .collect::<Result<Vec<_>>>()?,
            });
        }
    }
    Ok(branches)
}

pub fn commit_to_remote_commit(commit: &git::Commit) -> Result<RemoteCommit> {
    Ok(RemoteCommit {
        id: commit.id().to_string(),
        description: commit.message().unwrap_or_default().to_string(),
        created_at: commit.time().seconds().try_into().unwrap(),
        author: commit.author().into(),
    })
}
