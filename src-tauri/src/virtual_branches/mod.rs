pub mod branch;
mod iterator;
pub mod target;

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use std::{
    collections::{HashMap, HashSet},
    fmt, path, time, vec,
};

use anyhow::{bail, Context, Result};
use diffy::{apply_bytes, Patch};
use serde::Serialize;

pub use branch::{Branch, BranchCreateRequest, FileOwnership, Hunk, Ownership};
pub use iterator::BranchIterator as Iterator;
use uuid::Uuid;

use crate::{
    gb_repository,
    project_repository::{self, conflicts, diff},
    reader, sessions,
};

// this struct is a mapping to the view `Branch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of the Branch struct in Rust
// which is our persisted data structure for virtual branches
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranch {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
    pub commits: Vec<VirtualBranchCommit>,
    pub mergeable: bool,
    pub merge_conflicts: Vec<String>,
    pub conflicted: bool,
    pub order: usize,
    pub upstream: Option<String>,
    pub base_current: bool, // is this vbranch based on the current base branch?
}

// this is the struct that maps to the view `Commit` type in Typescript
// it is derived from walking the git commits between the `Branch.head` commit
// and the `Target.sha` commit, or, everything that is uniquely committed to
// the virtual branch we assign it to. an array of them are returned as part of
// the `VirtualBranch` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchCommit {
    pub id: String,
    pub description: String,
    pub created_at: u128,
    pub author_name: String,
    pub author_email: String,
    pub is_remote: bool,
}

// this struct is a mapping to the view `File` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of the
// `Branch.ownership` vector in Rust. an array of them are returned as part of
// the `VirtualBranch` struct, which map to each entry of the `Branch.ownership` vector
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchFile {
    pub id: String,
    pub path: path::PathBuf,
    pub hunks: Vec<VirtualBranchHunk>,
    pub modified_at: u128,
    pub conflicted: bool,
}

// this struct is a mapping to the view `Hunk` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of the
// each hunk in one `Branch.ownership` vector entry in Rust.
// an array of them are returned as part of the `VirtualBranchFile` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchHunk {
    pub id: String,
    pub diff: String,
    pub modified_at: u128,
    pub file_path: path::PathBuf,
    pub hash: String,
    pub start: usize,
    pub end: usize,
}

// this struct is a mapping to the view `RemoteBranch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
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
    pub last_commit_ts: u128,
    pub first_commit_ts: u128,
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<String>,
    pub authors: Vec<Author>,
    pub mergeable: bool,
    pub merge_conflicts: Vec<String>,
}

#[derive(Debug, Serialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub name: String,
    pub email: String,
    pub gravatar_url: url::Url,
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature) -> Self {
        let name = value.name().unwrap_or_default().to_string();
        let email = value.email().unwrap_or_default().to_string();

        let gravatar_url = url::Url::parse(&format!(
            "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
            md5::compute(email.to_lowercase())
        ))
        .unwrap();

        Author {
            name,
            email,
            gravatar_url,
        }
    }
}

fn get_default_target(current_session_reader: &sessions::Reader) -> Result<Option<target::Target>> {
    let target_reader = target::Reader::new(current_session_reader);
    match target_reader.read_default() {
        Ok(target) => Ok(Some(target)),
        Err(reader::Error::NotFound) => Ok(None),
        Err(e) => Err(e).context("failed to read default target"),
    }
}

pub fn apply_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
) -> Result<()> {
    if conflicts::is_resolving(project_repository) {
        bail!("cannot apply a branch, project is in a conflicted state");
    }
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let repo = &project_repository.git_repository;

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(()),
    };

    let virtual_branches = get_virtual_branches(gb_repository, Some(false))?;

    let writer = branch::Writer::new(gb_repository);

    let mut apply_branch = virtual_branches
        .iter()
        .find(|b| b.id == branch_id)
        .context("failed to find target branch")?
        .clone();
    let target_commit = repo
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;
    let target_tree = target_commit.tree().context("failed to get target tree")?;

    let mut branch_tree = repo
        .find_tree(apply_branch.tree)
        .context("failed to find branch tree")?;

    // calculate the merge base and make sure it's the same as the target commit
    // if not, we need to merge or rebase the branch to get it up to date

    let merge_options = git2::MergeOptions::new();

    let merge_base = repo.merge_base(default_target.sha, apply_branch.head)?;
    if merge_base != default_target.sha {
        // Branch is out of date, merge or rebase it
        let merge_base_tree = repo.find_commit(merge_base)?.tree()?;
        let mut merge_index = repo
            .merge_trees(
                &merge_base_tree,
                &branch_tree,
                &target_tree,
                Some(&merge_options),
            )
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            // currently we can only deal with the merge problem branch
            unapply_all_branches(gb_repository, project_repository)?;

            // apply the branch
            apply_branch.applied = true;
            writer.write(&apply_branch)?;

            // checkout the conflicts
            let mut checkout_options = git2::build::CheckoutBuilder::new();
            checkout_options
                .allow_conflicts(true)
                .conflict_style_merge(true)
                .force();
            repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;

            // mark conflicts
            let conflicts = merge_index.conflicts()?;
            let mut merge_conflicts = Vec::new();
            for path in conflicts.flatten() {
                if let Some(ours) = path.our {
                    let path = std::str::from_utf8(&ours.path)?.to_string();
                    merge_conflicts.push(path);
                }
            }
            conflicts::mark(
                project_repository,
                &merge_conflicts,
                Some(default_target.sha),
            )?;
            return Ok(());
        } else {
            let head_commit = repo
                .find_commit(apply_branch.head)
                .context("failed to find head commit")?;

            // commit our new upstream merge
            let (author, committer) = gb_repository.git_signatures()?;
            let message = "merge upstream";
            // write the merge commit
            let branch_tree_oid = merge_index.write_tree_to(repo)?;
            branch_tree = repo.find_tree(branch_tree_oid)?;

            let new_branch_head = repo.commit(
                None,
                &author,
                &committer,
                message,
                &branch_tree,
                &[&head_commit, &target_commit],
            )?;

            // ok, update the virtual branch
            apply_branch.head = new_branch_head;
            apply_branch.tree = branch_tree_oid;
            writer.write(&apply_branch)?;
        }
    }

    let wd_tree = get_wd_tree(repo)?;

    // check index for conflicts
    let mut merge_index = repo
        .merge_trees(&target_tree, &wd_tree, &branch_tree, Some(&merge_options))
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        bail!("vbranch has conflicts with other applied branches, sorry bro.");
    } else {
        // apply the branch
        apply_branch.applied = true;
        writer.write(&apply_branch)?;

        // checkout the merge index
        let mut checkout_options = git2::build::CheckoutBuilder::new();
        checkout_options.force();
        repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;
    }

    update_gitbutler_integration(gb_repository, project_repository)?;

    Ok(())
}

// to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
pub fn unapply_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
) -> Result<()> {
    if conflicts::is_resolving(project_repository) {
        bail!("cannot unapply, project is in a conflicted state");
    }
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(()),
    };

    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let mut target_branch = branch_reader
        .read(branch_id)
        .context("failed to read branch")?;

    if !target_branch.applied {
        bail!("branch is not applied");
    }

    let statuses = get_status_by_branch(gb_repository, project_repository)
        .context("failed to get status by branch")?;

    let status = statuses
        .iter()
        .find(|(s, _)| s.id == branch_id)
        .context("failed to find status for branch");

    let target_commit = gb_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let repo = &project_repository.git_repository;

    if let Ok((branch, files)) = status {
        let tree = write_tree(gb_repository, project_repository, branch, files)?;

        target_branch.tree = tree;
        target_branch.applied = false;
        branch_writer.write(&target_branch)?;
    }

    // ok, update the wd with the union of the rest of the branches
    let merge_options = git2::MergeOptions::new();
    let base_tree = target_commit.tree()?;
    let mut final_tree = target_commit.tree()?;

    // go through the other applied branches and merge them into the final tree
    // then check that out into the working directory
    for (branch, files) in statuses {
        if branch.id != branch_id {
            let tree_oid = write_tree(gb_repository, project_repository, &branch, &files)?;
            let branch_tree = repo.find_tree(tree_oid)?;
            if let Ok(mut result) =
                repo.merge_trees(&base_tree, &final_tree, &branch_tree, Some(&merge_options))
            {
                let final_tree_oid = result.write_tree_to(repo)?;
                final_tree = repo.find_tree(final_tree_oid)?;
            }
        }
    }
    // convert the final tree into an object
    let final_tree_oid = final_tree.id();
    let final_tree = repo.find_object(final_tree_oid, Some(git2::ObjectType::Tree))?;

    // checkout final_tree into the working directory
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force();
    repo.checkout_tree(&final_tree, Some(&mut checkout_options))?;

    update_gitbutler_integration(gb_repository, project_repository)?;

    Ok(())
}

fn unapply_all_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let applied_virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    for branch in applied_virtual_branches {
        let branch_id = branch.id;
        unapply_branch(gb_repository, project_repository, &branch_id)
            .context("failed to unapply branch")?;
    }

    Ok(())
}

pub fn remote_branches(
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
    let target_commit = repo
        .find_commit(main_oid)
        .context("failed to find target commit")?;

    let wd_tree = get_wd_tree(repo)?;

    let virtual_branches_names = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter_map(|branch| branch.upstream.map(|u| u.replace("refs/heads/", "")))
        .collect::<HashSet<_>>();
    let mut most_recent_branches_by_hash: HashMap<git2::Oid, (git2::Branch, u64)> = HashMap::new();

    for (branch, _) in repo.branches(None)?.flatten() {
        if let Some(branch_oid) = branch.get().target() {
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

            let branch_name = branch
                .name()
                .context("could not get branch name")?
                .context("could not get branch name")?
                .to_string();
            let branch_name = branch_name.replace("origin/", "");

            if virtual_branches_names.contains(&branch_name) {
                continue;
            }
            if branch_name == "HEAD" {
                continue;
            }
            if branch_name == "gitbutler/integration" {
                continue;
            }

            match most_recent_branches_by_hash.get(&branch_oid) {
                Some((_, existing_seconds)) => {
                    let branch_name = branch.get().name().context("could not get branch name")?;
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

    let mut most_recent_branches: Vec<(git2::Branch, u64)> =
        most_recent_branches_by_hash.into_values().collect();

    // take the most recent 20 branches
    most_recent_branches.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by timestamp in descending order.
    let sorted_branches: Vec<git2::Branch> = most_recent_branches
        .into_iter()
        .map(|(branch, _)| branch)
        .collect();
    let top_branches = sorted_branches.into_iter().take(20).collect::<Vec<_>>(); // Take the first 20 entries.

    let mut branches: Vec<RemoteBranch> = Vec::new();
    for branch in &top_branches {
        let branch_name = branch.get().name().context("could not get branch name")?;
        let upstream_branch = branch.upstream();
        match branch.get().target() {
            Some(branch_oid) => {
                // get the branch ref
                let branch_commit = repo
                    .find_commit(branch_oid)
                    .context("failed to find branch commit")?;

                let count_behind = project_repository
                    .distance(main_oid, branch_oid)
                    .context("failed to get behind count")?;

                let ahead = project_repository
                    .log(branch_oid, main_oid)
                    .context("failed to get ahead commits")?;
                let count_ahead = ahead.len();

                let min_time = ahead.iter().map(|commit| commit.time().seconds()).min();
                let max_time = ahead.iter().map(|commit| commit.time().seconds()).max();
                let authors = ahead
                    .iter()
                    .map(|commit| commit.author())
                    .map(Author::from)
                    .collect::<HashSet<_>>();

                let upstream = match upstream_branch {
                    Ok(upstream_branch) => {
                        upstream_branch.get().name().map(|name| name.to_string())
                    }
                    Err(_) => None,
                };

                if count_ahead > 0 {
                    if let Ok(base_tree) = find_base_tree(repo, &branch_commit, &target_commit) {
                        // determine if this tree is mergeable
                        let branch_tree = branch_commit.tree()?;
                        let (mergeable, merge_conflicts) =
                            check_mergeable(repo, &base_tree, &branch_tree, &wd_tree)?;

                        branches.push(RemoteBranch {
                            sha: branch_oid.to_string(),
                            name: branch_name.to_string(),
                            upstream,
                            last_commit_ts: max_time
                                .unwrap_or(0)
                                .try_into()
                                .context("failed to convert i64 to u128")?,
                            first_commit_ts: min_time
                                .unwrap_or(0)
                                .try_into()
                                .context("failed to convert i64 to u128")?,
                            ahead: count_ahead
                                .try_into()
                                .context("failed to convert usize to u32")?,
                            behind: count_behind,
                            authors: authors.into_iter().collect(),
                            mergeable,
                            merge_conflicts,
                        });
                    };
                }
            }
            None => {
                // this is a detached head
                branches.push(RemoteBranch {
                    sha: "".to_string(),
                    name: branch_name.to_string(),
                    last_commit_ts: 0,
                    first_commit_ts: 0,
                    ahead: 0,
                    behind: 0,
                    upstream: None,
                    authors: vec![],
                    mergeable: false,
                    merge_conflicts: vec![],
                });
            }
        }
    }
    Ok(branches)
}

fn get_wd_tree(repo: &git2::Repository) -> Result<git2::Tree> {
    let mut index = repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    Ok(tree)
}

fn find_base_tree<'a>(
    repo: &'a git2::Repository,
    branch_commit: &'a git2::Commit<'a>,
    target_commit: &'a git2::Commit<'a>,
) -> Result<git2::Tree<'a>> {
    // find merge base between target_commit and branch_commit
    let merge_base = repo
        .merge_base(target_commit.id(), branch_commit.id())
        .context("failed to find merge base")?;
    // turn oid into a commit
    let merge_base_commit = repo
        .find_commit(merge_base)
        .context("failed to find merge base commit")?;
    let base_tree = merge_base_commit
        .tree()
        .context("failed to get base tree object")?;
    Ok(base_tree.clone())
}

fn check_mergeable(
    repo: &git2::Repository,
    base_tree: &git2::Tree,
    branch_tree: &git2::Tree,
    wd_tree: &git2::Tree,
) -> Result<(bool, Vec<String>)> {
    let mut merge_conflicts = Vec::new();

    let merge_options = git2::MergeOptions::new();
    let merge_index = repo
        .merge_trees(base_tree, wd_tree, branch_tree, Some(&merge_options))
        .context("failed to merge trees")?;
    let mergeable = !merge_index.has_conflicts();
    if merge_index.has_conflicts() {
        let conflicts = merge_index.conflicts()?;
        for path in conflicts.flatten() {
            if let Some(their) = path.their {
                let path = std::str::from_utf8(&their.path)?.to_string();
                merge_conflicts.push(path);
            } else if let Some(ours) = path.our {
                let path = std::str::from_utf8(&ours.path)?.to_string();
                merge_conflicts.push(path);
            } else if let Some(anc) = path.ancestor {
                let path = std::str::from_utf8(&anc.path)?.to_string();
                merge_conflicts.push(path);
            }
        }
    }
    Ok((mergeable, merge_conflicts))
}

pub fn list_virtual_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    with_commits: bool,
) -> Result<Vec<VirtualBranch>> {
    let mut branches: Vec<VirtualBranch> = Vec::new();
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;

    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session reader")?;

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(vec![]),
    };

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<_>>();

    let statuses = get_status_by_branch(gb_repository, project_repository)?;

    let repo = &project_repository.git_repository;
    let wd_tree = get_wd_tree(repo)?;

    for branch in &virtual_branches {
        let branch_statuses = statuses.clone();
        let mut files: Vec<VirtualBranchFile> = vec![];
        //let (branch, files) in &statuses
        // find the entry in statuses with this branch id
        let maybe_status = branch_statuses
            .into_iter()
            .find(|(vbranch, _)| vbranch.id == branch.id);

        if let Some((_vbranch, sfiles)) = maybe_status {
            files = sfiles.clone();
        }

        let mut vfiles = vec![];

        // check if head tree does not match target tree
        // if so, we diff the head tree and the new write_tree output to see what is new and filter the hunks to just those
        if (default_target.sha != branch.head) && branch.applied && with_commits {
            // TODO: make sure this works
            let vtree = write_tree(gb_repository, project_repository, branch, &files)?;
            let repo = &project_repository.git_repository;
            // get the trees
            let commit_old = repo.find_commit(branch.head)?;
            let tree_old = commit_old.tree()?;
            let vtree_tree = repo.find_tree(vtree)?;

            // do a diff between branch.head and the tree we _would_ commit
            let diff = diff::trees(project_repository, &tree_old, &vtree_tree)
                .context("failed to diff trees")?;
            let hunks_by_filepath = hunks_by_filepath(project_repository, &diff);

            vfiles = hunks_by_filepath
                .into_iter()
                .map(|(file_path, hunks)| VirtualBranchFile {
                    id: file_path.display().to_string(),
                    path: file_path.clone(),
                    hunks: hunks.clone(),
                    modified_at: hunks.iter().map(|h| h.modified_at).max().unwrap_or(0),
                    conflicted: conflicts::is_conflicting(
                        project_repository,
                        Some(&file_path.display().to_string()),
                    )
                    .unwrap_or(false),
                })
                .collect::<Vec<_>>();
        } else {
            for file in files {
                vfiles.push(file.clone());
            }
        }

        let repo = &project_repository.git_repository;

        // see if we can identify some upstream
        let mut upstream_commit = None;
        if let Some(branch_upstream) = &branch.upstream {
            // get the target remote
            let remotes = repo.remotes()?;
            let mut upstream_remote = None;
            for remote_name in remotes.iter() {
                let remote_name = match remote_name {
                    Some(name) => name,
                    None => continue,
                };

                let remote = repo.find_remote(remote_name)?;
                let url = match remote.url() {
                    Some(url) => url,
                    None => continue,
                };

                if url == default_target.remote_url {
                    upstream_remote = Some(remote);
                    break;
                }
            }
            if let Some(remote) = upstream_remote {
                // remove "refs/heads/" from the branch name
                let branch_name = branch_upstream.replace("refs/heads/", "");
                let full_branch_name =
                    format!("refs/remotes/{}/{}", remote.name().unwrap(), branch_name);
                if let Ok(upstream_oid) = repo.refname_to_id(&full_branch_name) {
                    if let Ok(upstream_commit_obj) = repo.find_commit(upstream_oid) {
                        upstream_commit = Some(upstream_commit_obj);
                    }
                }
            }
        }

        // find upstream commits if we found an upstream reference
        let mut upstream_commits = HashMap::new();
        if let Some(ref upstream) = upstream_commit {
            let merge_base =
                repo.merge_base(upstream.id(), default_target.sha)
                    .context(format!(
                        "failed to find merge base between {} and {}",
                        upstream.id(),
                        default_target.sha
                    ))?;
            for oid in project_repository.l(upstream.id(), merge_base)? {
                upstream_commits.insert(oid, true);
            }
        }

        // find all commits on head that are not on target.sha
        let mut commits = vec![];
        for commit in project_repository
            .log(branch.head, default_target.sha)
            .context(format!("failed to get log for branch {}", branch.name))?
        {
            let timestamp = commit.time().seconds() as u128;
            let signature = commit.author();
            let name = signature.name().unwrap().to_string();
            let email = signature.email().unwrap().to_string();
            let message = commit.message().unwrap().to_string();
            let sha = commit.id().to_string();
            let is_remote = upstream_commits.contains_key(&commit.id());

            let commit = VirtualBranchCommit {
                id: sha,
                created_at: timestamp * 1000,
                author_name: name,
                author_email: email,
                description: message,
                is_remote,
            };
            commits.push(commit);
        }

        let mut mergeable = true;
        let mut merge_conflicts = vec![];
        let mut base_current = true;
        if !branch.applied {
            // determine if this branch is up to date with the target/base
            let merge_base = repo.merge_base(default_target.sha, branch.head)?;
            if merge_base != default_target.sha {
                base_current = false;
                mergeable = false;
            } else {
                let target_commit = repo
                    .find_commit(default_target.sha)
                    .context("failed to find target commit")?;
                let branch_commit = repo
                    .find_commit(branch.head)
                    .context("failed to find branch commit")?;
                if let Ok(base_tree) = find_base_tree(repo, &branch_commit, &target_commit) {
                    // determine if this tree is mergeable
                    let branch_tree = repo
                        .find_tree(branch.tree)
                        .context("failed to find branch tree")?;
                    (mergeable, merge_conflicts) =
                        check_mergeable(repo, &base_tree, &branch_tree, &wd_tree)
                            .context("failed to check mergeable")?;
                } else {
                    // there is no common base
                    mergeable = false;
                };
            }
        }

        let branch = VirtualBranch {
            id: branch.id.to_string(),
            name: branch.name.to_string(),
            active: branch.applied,
            files: vfiles,
            order: branch.order,
            commits,
            mergeable,
            merge_conflicts,
            upstream: branch
                .upstream
                .as_ref()
                .map(|u| u.replace("refs/heads/", "")),
            conflicted: conflicts::is_resolving(project_repository),
            base_current,
        };
        branches.push(branch);
    }
    branches.sort_by(|a, b| a.order.cmp(&b.order));
    Ok(branches)
}

pub fn create_virtual_branch_from_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_ref: &str,
) -> Result<String> {
    let name = branch_ref
        .replace("refs/heads/", "")
        .replace("refs/remotes/", "")
        .replace("origin/", ""); // TODO: get this properly
    let upstream = Some(format!("refs/heads/{}", &name));
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = get_default_target(&current_session_reader)
        .context("failed to get default target")?
        .context("no default target found")?;

    let repo = &project_repository.git_repository;
    let head = repo.revparse_single(branch_ref)?;
    let head_commit = head.peel_to_commit()?;
    let tree = head_commit.tree().context("failed to find tree")?;

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let order = virtual_branches.len();

    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get elapsed time")?
        .as_millis();

    let branch_id = Uuid::new_v4().to_string();
    let mut branch = Branch {
        id: branch_id.clone(),
        name,
        applied: false,
        upstream,
        tree: tree.id(),
        head: head_commit.id(),
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: Ownership::default(),
        order,
    };

    // add file ownership based off the diff
    let target_commit = repo.find_commit(default_target.sha)?;
    let merge_base = repo.merge_base(target_commit.id(), head_commit.id())?;
    let merge_tree = repo.find_commit(merge_base)?.tree()?;
    if merge_base != target_commit.id() {
        let target_tree = target_commit.tree()?;
        let head_tree = head_commit.tree()?;

        // merge target and head
        let merge_options = git2::MergeOptions::new();
        let mut merge_index = repo
            .merge_trees(&merge_tree, &head_tree, &target_tree, Some(&merge_options))
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            bail!("merge conflict");
        } else {
            let (author, committer) = gb_repository.git_signatures()?;
            let new_head_tree_oid = merge_index
                .write_tree_to(repo)
                .context("failed to write merge tree")?;
            let new_head_tree = repo
                .find_tree(new_head_tree_oid)
                .context("failed to find tree")?;

            let new_branch_head = repo.commit(
                None,
                &author,
                &committer,
                "merged upstream",
                &new_head_tree,
                &[&head_commit, &target_commit],
            )?;
            branch.head = new_branch_head;
            branch.tree = new_head_tree_oid
        }
    }

    // do a diff between the head of this branch and the target base
    let diff =
        diff::trees(project_repository, &merge_tree, &tree).context("failed to diff trees")?;
    let hunks_by_filepath = hunks_by_filepath(project_repository, &diff);

    // assign ownership to the branch
    for hunk in hunks_by_filepath.values().flatten() {
        branch.ownership.put(
            &FileOwnership::try_from(format!("{}:{}", hunk.file_path.display(), hunk.id)).unwrap(),
        );
    }

    let writer = branch::Writer::new(gb_repository);
    writer.write(&branch).context("failed to write branch")?;
    Ok(branch_id)
}

pub fn create_virtual_branch(
    gb_repository: &gb_repository::Repository,
    create: &BranchCreateRequest,
) -> Result<branch::Branch> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = target_reader
        .read_default()
        .context("failed to read default")?;

    let repo = &gb_repository.git_repository;
    let commit = repo
        .find_commit(default_target.sha)
        .context("failed to find commit")?;
    let tree = commit.tree().context("failed to find tree")?;

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<branch::Branch>>();
    virtual_branches.sort_by_key(|branch| branch.order);

    let order = if let Some(order) = create.order {
        if order > virtual_branches.len() {
            virtual_branches.len()
        } else {
            order
        }
    } else {
        virtual_branches.len()
    };
    let branch_writer = branch::Writer::new(gb_repository);

    // make space for the new branch
    for branch in virtual_branches.iter().skip(order) {
        let mut branch = branch.clone();
        branch.order += 1;
        branch_writer
            .write(&branch)
            .context("failed to write branch")?;
    }

    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get elapsed time")?
        .as_millis();

    let name: String = create
        .name
        .as_ref()
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("Virtual branch {}", virtual_branches.len() + 1));

    let mut branch = Branch {
        id: Uuid::new_v4().to_string(),
        name,
        applied: true,
        upstream: None,
        tree: tree.id(),
        head: default_target.sha,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: Ownership::default(),
        order,
    };

    if let Some(ownership) = &create.ownership {
        let branch_reader = branch::Reader::new(&current_session_reader);
        set_ownership(&branch_reader, &branch_writer, &mut branch, ownership)
            .context("failed to set ownership")?;
    }

    branch_writer
        .write(&branch)
        .context("failed to write branch")?;

    Ok(branch)
}

pub fn update_branch(
    gb_repository: &gb_repository::Repository,
    branch_update: branch::BranchUpdateRequest,
) -> Result<branch::Branch> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let mut branch = branch_reader
        .read(&branch_update.id)
        .context("failed to read branch")?;

    if let Some(ownership) = branch_update.ownership {
        set_ownership(&branch_reader, &branch_writer, &mut branch, &ownership)
            .context("failed to set ownership")?;
    }

    if let Some(name) = branch_update.name {
        branch.name = name;
    };

    if let Some(order) = branch_update.order {
        branch.order = order;
    };

    branch_writer
        .write(&branch)
        .context("failed to write target branch")?;

    Ok(branch)
}

pub fn delete_branch(
    gb_repository: &gb_repository::Repository,
    branch_id: &str,
) -> Result<branch::Branch> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let branch = branch_reader
        .read(branch_id)
        .context("failed to read branch")?;

    branch_writer
        .delete(&branch)
        .context("Failed to remove branch")?;

    Ok(branch)
}

fn set_ownership(
    branch_reader: &branch::Reader,
    branch_writer: &branch::Writer,
    target_branch: &mut branch::Branch,
    ownership: &branch::Ownership,
) -> Result<()> {
    if target_branch.ownership.eq(ownership) {
        // nothing to update
        return Ok(());
    }

    let mut virtual_branches = Iterator::new(branch_reader.reader())
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .filter(|branch| branch.id != target_branch.id)
        .collect::<Vec<_>>();

    for file_ownership in &ownership.files {
        for branch in &mut virtual_branches {
            let taken = branch.ownership.take(file_ownership);
            if !taken.is_empty() {
                branch_writer.write(branch).context(format!(
                    "failed to write source branch for {}",
                    file_ownership
                ))?;
            }
        }
    }

    target_branch.ownership = ownership.clone();

    Ok(())
}

fn get_mtime(cache: &mut HashMap<path::PathBuf, u128>, file_path: &path::PathBuf) -> u128 {
    match cache.get(file_path) {
        Some(mtime) => *mtime,
        None => {
            let mtime = file_path
                .metadata()
                .map(|metadata| {
                    metadata
                        .modified()
                        .or(metadata.created())
                        .unwrap_or_else(|_| time::SystemTime::now())
                })
                .unwrap_or_else(|_| time::SystemTime::now())
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            cache.insert(file_path.to_path_buf(), mtime);
            mtime
        }
    }
}

fn diff_hash(diff: &str) -> String {
    let addition = diff
        .lines()
        .skip(1)
        .filter(|line| line.starts_with('+'))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{:x}", md5::compute(addition))
}

fn hunks_by_filepath(
    project_repository: &project_repository::Repository,
    diff: &HashMap<path::PathBuf, Vec<diff::Hunk>>,
) -> HashMap<path::PathBuf, Vec<VirtualBranchHunk>> {
    let mut mtimes: HashMap<path::PathBuf, u128> = HashMap::new();
    diff.iter()
        .map(|(file_path, hunks)| {
            let hunks = hunks
                .iter()
                .map(|hunk| VirtualBranchHunk {
                    id: format!("{}-{}", hunk.new_start, hunk.new_start + hunk.new_lines),
                    modified_at: get_mtime(&mut mtimes, &project_repository.path().join(file_path)),
                    file_path: file_path.clone(),
                    diff: hunk.diff.clone(),
                    start: hunk.new_start,
                    end: hunk.new_start + hunk.new_lines,
                    hash: diff_hash(&hunk.diff),
                })
                .collect::<Vec<_>>();
            (file_path.clone(), hunks)
        })
        .collect::<HashMap<_, _>>()
}

// list the virtual branches and their file statuses (statusi?)
pub fn get_status_by_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository<'_>,
) -> Result<Vec<(branch::Branch, Vec<VirtualBranchFile>)>> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = match get_default_target(&current_session_reader) {
        Ok(Some(target)) => Ok(target),
        Ok(None) => {
            return Ok(vec![]);
        }
        Err(e) => Err(e),
    }
    .context("failed to read default target")?;

    let diff = diff::workdir(
        project_repository,
        &default_target.sha,
        &diff::Options::default(),
    )
    .context("failed to diff")?;
    let mut hunks_by_filepath = hunks_by_filepath(project_repository, &diff);

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    // sort by order, so that the default branch is first (left in the ui)
    virtual_branches.sort_by(|a, b| a.order.cmp(&b.order));

    if virtual_branches.is_empty() && !hunks_by_filepath.is_empty() {
        // no virtual branches, but hunks: create default branch
        virtual_branches =
            vec![
                create_virtual_branch(gb_repository, &BranchCreateRequest::default())
                    .context("failed to default branch")?,
            ];
    }

    // align branch ownership to the real hunks:
    // - update shifted hunks
    // - remove non existent hunks

    let mut hunks_by_branch_id: HashMap<String, Vec<VirtualBranchHunk>> = virtual_branches
        .iter()
        .map(|branch| (branch.id.clone(), vec![]))
        .collect();

    for branch in &mut virtual_branches {
        let mut updated: Vec<_> = vec![];
        branch.ownership = Ownership {
            files: branch
                .ownership
                .files
                .iter()
                .filter_map(|file_owership| {
                    let current_hunks = match hunks_by_filepath.get_mut(&file_owership.file_path) {
                        None => {
                            // if the file is not in the diff, we don't want it
                            return None;
                        }
                        Some(hunks) => hunks,
                    };
                    let updated_hunks: Vec<Hunk> = file_owership
                        .hunks
                        .iter()
                        .filter_map(|owned_hunk| {
                            // if any of the current hunks intersects with the owned hunk, we want to keep it
                            for (i, current_hunk) in current_hunks.iter().enumerate() {
                                let ch = Hunk::new(
                                    current_hunk.start,
                                    current_hunk.end,
                                    Some(current_hunk.hash.clone()),
                                    Some(current_hunk.modified_at),
                                )
                                .unwrap();
                                if owned_hunk.eq(&ch) {
                                    // try to re-use old timestamp
                                    let timestamp = owned_hunk
                                        .timestam_ms()
                                        .unwrap_or(current_hunk.modified_at);

                                    // push hunk to the end of the list, preserving the order
                                    hunks_by_branch_id
                                        .entry(branch.id.clone())
                                        .or_default()
                                        .push(VirtualBranchHunk {
                                            id: ch.with_timestamp(timestamp).to_string(),
                                            modified_at: timestamp,
                                            ..current_hunk.clone()
                                        });

                                    // remove the hunk from the current hunks because each hunk can
                                    // only be owned once
                                    current_hunks.remove(i);

                                    return Some(owned_hunk.with_timestamp(timestamp));
                                } else if owned_hunk.intersects(&ch) {
                                    // if it's an intersection, push the hunk to the beginning,
                                    // indicating the the hunk has been updated
                                    hunks_by_branch_id
                                        .entry(branch.id.clone())
                                        .or_default()
                                        .insert(
                                            0,
                                            VirtualBranchHunk {
                                                id: ch.to_string(),
                                                ..current_hunk.clone()
                                            },
                                        );

                                    // track updated hunks to bubble them up later
                                    updated.push(FileOwnership {
                                        file_path: file_owership.file_path.clone(),
                                        hunks: vec![ch.clone()],
                                    });

                                    // remove the hunk from the current hunks because each hunk can
                                    // only be owned once
                                    current_hunks.remove(i);

                                    // return updated version, with new hash and/or timestamp
                                    return Some(ch);
                                }
                            }
                            None
                        })
                        .collect();

                    if updated_hunks.is_empty() {
                        // if there are no hunks left, we don't want the file either
                        None
                    } else {
                        Some(FileOwnership {
                            file_path: file_owership.file_path.clone(),
                            hunks: updated_hunks,
                        })
                    }
                })
                .collect(),
        };

        // add the updated hunks to the branch again to promote them to the top
        updated
            .iter()
            .for_each(|file_ownership| branch.ownership.put(file_ownership));
    }

    // put the remaining hunks into the default (first) branch
    for hunk in hunks_by_filepath.values().flatten() {
        virtual_branches[0].ownership.put(
            &FileOwnership::try_from(format!(
                "{}:{}-{}-{}-{}",
                hunk.file_path.display(),
                hunk.start,
                hunk.end,
                hunk.hash,
                hunk.modified_at,
            ))
            .unwrap(),
        );
        hunks_by_branch_id
            .entry(virtual_branches[0].id.clone())
            .or_default()
            .push(hunk.clone());
    }

    // write updated state
    let branch_writer = branch::Writer::new(gb_repository);
    for vranch in &virtual_branches {
        branch_writer
            .write(vranch)
            .context(format!("failed to write virtual branch {}", vranch.name))?;
    }

    let mut statuses: Vec<(branch::Branch, Vec<VirtualBranchFile>)> = vec![];
    for (branch_id, hunks) in hunks_by_branch_id {
        let branch = virtual_branches
            .iter()
            .find(|b| b.id.eq(&branch_id))
            .unwrap()
            .clone();

        let mut files = hunks
            .iter()
            .fold(HashMap::<path::PathBuf, Vec<_>>::new(), |mut acc, hunk| {
                acc.entry(hunk.file_path.clone())
                    .or_default()
                    .push(hunk.clone());
                acc
            })
            .into_iter()
            .map(|(file_path, hunks)| VirtualBranchFile {
                id: file_path.display().to_string(),
                path: file_path.clone(),
                hunks: hunks.clone(),
                modified_at: hunks.iter().map(|h| h.modified_at).max().unwrap_or(0),
                conflicted: conflicts::is_conflicting(
                    project_repository,
                    Some(&file_path.display().to_string()),
                )
                .unwrap_or(false),
            })
            .collect::<Vec<_>>();

        files.sort_by(|a, b| {
            branch
                .ownership
                .files
                .iter()
                .position(|o| o.file_path.eq(&a.path))
                .unwrap_or(999)
                .cmp(
                    &branch
                        .ownership
                        .files
                        .iter()
                        .position(|id| id.file_path.eq(&b.path))
                        .unwrap_or(999),
                )
        });

        statuses.push((branch, files));
    }

    statuses.sort_by(|a, b| a.0.order.cmp(&b.0.order));

    Ok(statuses)
}

// try to update the target branch
// this means that we need to:
// determine if what the target branch is now pointing to is mergeable with our current working directory
// merge the target branch into our current working directory
// update the target sha
pub fn update_branch_target(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    // look up the target and see if there is a new oid
    let mut target = get_default_target(&current_session_reader)
        .context("failed to get target")?
        .context("no target found")?;

    let branch_reader = branch::Reader::new(&current_session_reader);
    let writer = branch::Writer::new(gb_repository);

    let repo = &project_repository.git_repository;
    let branch = repo
        .find_branch(&target.branch_name, git2::BranchType::Remote)
        .context(format!("failed to find branch {}", target.branch_name))?;
    let new_target_commit = branch.get().peel_to_commit().context(format!(
        "failed to peel branch {} to commit",
        target.branch_name
    ))?;
    let new_target_oid = new_target_commit.id();

    // if the target has not changed, do nothing
    if new_target_oid == target.sha {
        return Ok(());
    }

    // ok, target has changed, so now we need to merge it into our current work and update our branches

    // get all virtual branches, we need to try to update them all
    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<_>>();

    let merge_options = git2::MergeOptions::new();

    // get tree from new target
    let new_target_commit = repo.find_commit(new_target_oid)?;
    let new_target_tree = new_target_commit.tree()?;

    // get tree from target.sha
    let target_commit = repo.find_commit(target.sha)?;
    let target_tree = target_commit.tree()?;

    let (author, committer) = gb_repository.git_signatures()?;

    // ok, now we need to deal with a number of situations
    // 1. applied branch, uncommitted conflicts
    // 2. applied branch, committed conflicts but not uncommitted
    // 3. applied branch, no conflicts
    // 4. unapplied branch, uncommitted conflicts
    // 5. unapplied branch, committed conflicts but not uncommitted
    // 6. unapplied branch, no conflicts

    let mut vbranches = list_virtual_branches(gb_repository, project_repository, false)?;
    let mut vbranches_commits = list_virtual_branches(gb_repository, project_repository, true)?;
    // update the heads of all our virtual branches
    for virtual_branch in &mut virtual_branches {
        let mut virtual_branch = virtual_branch.clone();

        // get the matching vbranch
        let vbranch = vbranches
            .iter()
            .find(|vbranch| vbranch.id == virtual_branch.id)
            .unwrap();

        let vbranch_commits = vbranches_commits
            .iter()
            .find(|vbranch| vbranch.id == virtual_branch.id)
            .unwrap();

        let mut tree_oid = virtual_branch.tree;
        if virtual_branch.applied {
            tree_oid = write_tree(
                gb_repository,
                project_repository,
                &virtual_branch,
                &vbranch.files,
            )?;
        }
        let branch_tree = repo.find_tree(tree_oid)?;

        // check for conflicts with this tree
        let merge_index = repo
            .merge_trees(
                &target_tree,
                &branch_tree,
                &new_target_tree,
                Some(&merge_options),
            )
            .context("failed to merge trees")?;

        // check if the branch head has conflicts
        if merge_index.has_conflicts() {
            // unapply branch for now
            if virtual_branch.applied {
                // this changes the wd, and thus the hunks, so we need to re-run the active branch listing
                unapply_branch(gb_repository, project_repository, &virtual_branch.id)?;
                vbranches = list_virtual_branches(gb_repository, project_repository, false)?;
                vbranches_commits = list_virtual_branches(gb_repository, project_repository, true)?;
            }
            virtual_branch = branch_reader.read(&virtual_branch.id)?;

            if target.sha != virtual_branch.head {
                // check if the head conflicts
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head)?;
                let head_tree = head_commit.tree()?;

                let mut merge_index = repo
                    .merge_trees(
                        &target_tree,
                        &head_tree,
                        &new_target_tree,
                        Some(&merge_options),
                    )
                    .context("failed to merge trees")?;

                // check index for conflicts
                // if it has conflicts, we just ignore it
                if !merge_index.has_conflicts() {
                    // does not conflict with head, so lets merge it and update the head
                    let merge_tree_oid = merge_index
                        .write_tree_to(repo)
                        .context("failed to write tree")?;
                    // get tree from merge_tree_oid
                    let merge_tree = repo
                        .find_tree(merge_tree_oid)
                        .context("failed to find tree")?;

                    // commit the merge tree oid
                    let new_branch_head = repo.commit(
                        None,
                        &author,
                        &committer,
                        "merged upstream (head only)",
                        &merge_tree,
                        &[&head_commit, &new_target_commit],
                    )?;
                    virtual_branch.head = new_branch_head;
                    virtual_branch.tree = merge_tree_oid;
                    writer.write(&virtual_branch)?;
                }
            }
        } else {
            // branch head does not have conflicts, so don't unapply it, but still try to merge it's head if there are commits
            // but also remove/archive it if the branch is fully integrated
            if target.sha == virtual_branch.head {
                // there were no conflicts and no commits, so just update the head
                virtual_branch.head = new_target_oid;
                virtual_branch.tree = tree_oid;
                writer.write(&virtual_branch)?;
            } else {
                // no conflicts, but there have been commits, so update head with a merge
                // there are commits on this branch, so create a merge commit with the new tree
                // get tree from virtual branch head
                let head_commit = repo.find_commit(virtual_branch.head)?;
                let head_tree = repo.find_tree(virtual_branch.tree)?;

                let mut merge_index = repo
                    .merge_trees(
                        &target_tree,
                        &head_tree,
                        &new_target_tree,
                        Some(&merge_options),
                    )
                    .context("failed to merge trees")?;

                // check index for conflicts
                if merge_index.has_conflicts() {
                    // unapply branch for now
                    // this changes the wd, and thus the hunks, so we need to re-run the active branch listing
                    unapply_branch(gb_repository, project_repository, &virtual_branch.id)?;
                    vbranches = list_virtual_branches(gb_repository, project_repository, false)?;
                    vbranches_commits =
                        list_virtual_branches(gb_repository, project_repository, true)?;
                } else {
                    // get the merge tree oid from writing the index out
                    let merge_tree_oid = merge_index
                        .write_tree_to(repo)
                        .context("failed to write tree")?;
                    // get tree from merge_tree_oid
                    let merge_tree = repo
                        .find_tree(merge_tree_oid)
                        .context("failed to find tree")?;

                    // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
                    // then the vbranch is fully merged, so delete it
                    if merge_tree_oid == new_target_tree.id() && vbranch_commits.files.is_empty() {
                        writer.delete(&virtual_branch)?;
                    } else {
                        // commit the merge tree oid
                        let new_branch_head = repo.commit(
                            None,
                            &author,
                            &committer,
                            "merged upstream",
                            &merge_tree,
                            &[&head_commit, &new_target_commit],
                        )?;
                        virtual_branch.head = new_branch_head;
                        virtual_branch.tree = merge_tree_oid;
                        writer.write(&virtual_branch)?;
                    }
                }
            }
        }
    }

    // ok, now all the problematic branches have been unapplied, so we can try to merge the upstream branch into our current working directory
    // first, get a new wd tree
    let wd_tree = get_wd_tree(repo)?;

    // and try to merge it
    let mut merge_index = repo
        .merge_trees(
            &target_tree,
            &wd_tree,
            &new_target_tree,
            Some(&merge_options),
        )
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        bail!("this should not have happened, we should have already detected this");
    }

    // now we can try to merge the upstream branch into our current working directory
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force();
    repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;

    // write new target oid
    target.sha = new_target_oid;
    let target_writer = target::Writer::new(gb_repository);
    target_writer.write_default(&target)?;

    update_gitbutler_integration(gb_repository, project_repository)?;

    Ok(())
}

fn get_target(gb_repository: &gb_repository::Repository) -> Result<target::Target> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target = get_default_target(&current_session_reader)
        .context("failed to read default target")?
        .context("failed to read default target")?;
    Ok(default_target)
}

fn get_virtual_branches(
    gb_repository: &gb_repository::Repository,
    applied: Option<bool>,
) -> Result<Vec<branch::Branch>> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let applied_virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied == applied.unwrap_or(true))
        .collect::<Vec<_>>();
    Ok(applied_virtual_branches)
}

pub fn update_gitbutler_integration(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<()> {
    let target = get_target(gb_repository)?;
    let repo = &project_repository.git_repository;

    // write the currrent target sha to a temp branch as a parent
    let my_ref = "refs/heads/gitbutler/integration";
    repo.reference(my_ref, target.sha, true, "update target")?;

    // get commit object from target.sha
    let target_commit = repo.find_commit(target.sha)?;

    // get current repo head for reference
    let head = repo.head()?;
    let mut prev_head = head.name().unwrap().to_string();
    let mut prev_sha = head.target().unwrap().to_string();
    let mut integration_file = repo.path().to_path_buf();
    integration_file.push("integration");
    if prev_head != my_ref {
        // we are moving from a regular branch to our gitbutler integration branch, save the original
        // write a file to .git/integration with the previous head and name
        let mut file = std::fs::File::create(integration_file)?;
        prev_head.push(':');
        prev_head.push_str(&prev_sha);
        file.write_all(prev_head.as_bytes())?;
    } else {
        // read the .git/integration file
        if let Ok(mut integration_file) = std::fs::File::open(integration_file) {
            let mut prev_data = String::new();
            integration_file.read_to_string(&mut prev_data)?;
            let parts: Vec<&str> = prev_data.split(':').collect();

            prev_head = parts[0].to_string();
            prev_sha = parts[1].to_string();
        }
    }

    // commit index to temp head for the merge
    repo.set_head(my_ref).context("failed to set head")?;

    // get all virtual branches, we need to try to update them all
    let applied_virtual_branches = get_virtual_branches(gb_repository, Some(true))?;

    let merge_options = git2::MergeOptions::new();
    let base_tree = target_commit.tree()?;
    let mut final_tree = target_commit.tree()?;
    for branch in &applied_virtual_branches {
        // merge this branches tree with our tree
        let branch_head = repo.find_commit(branch.head)?;
        let branch_tree = branch_head.tree()?;
        if let Ok(mut result) =
            repo.merge_trees(&base_tree, &final_tree, &branch_tree, Some(&merge_options))
        {
            if !result.has_conflicts() {
                let final_tree_oid = result.write_tree_to(repo)?;
                final_tree = repo.find_tree(final_tree_oid)?;
            }
        }
    }

    let (author, committer) = gb_repository.git_signatures()?;

    // message that says how to get back to where they were
    let mut message = "GitButler Integration Commit".to_string();
    message.push_str("\n\n");
    message.push_str(
        "This is an integration commit for the virtual branches that GitButler is tracking.\n",
    );
    message.push_str(
        "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
    );
    message.push_str(
        "forth easily. If you switch to another branch, GitButler will need to be reinitialized.\n",
    );
    message.push_str("If you commit on this branch, GitButler will throw it away.\n\n");
    message.push_str("Here are the branches that are currently applied:\n");
    for branch in &applied_virtual_branches {
        message.push_str(" - ");
        message.push_str(branch.name.as_str());
        message.push('\n');
        for file in &branch.ownership.files {
            message.push_str("   - ");
            message.push_str(&file.file_path.display().to_string());
            message.push('\n');
        }
    }
    message.push_str("\nTo get back to where you were, run:\n\n");
    message.push_str("git checkout ");
    message.push_str(&prev_head);
    message.push_str("\n\n");
    message.push_str("The sha for that commit was: ");
    message.push_str(&prev_sha);

    repo.commit(
        Some("HEAD"),
        &author,
        &committer,
        &message,
        &final_tree,
        &[&target_commit],
    )?;
    Ok(())
}

// this function takes a list of file ownership,
// constructs a tree from those changes on top of the target
// and writes it as a new tree for storage
fn write_tree(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    _vbranch: &branch::Branch,
    files: &Vec<VirtualBranchFile>,
) -> Result<git2::Oid> {
    // read the base sha into an index
    let target = get_target(gb_repository)?;
    let git_repository = &project_repository.git_repository;

    let head_commit = git_repository.find_commit(target.sha)?;
    let base_tree = head_commit.tree()?;

    let mut builder = git2::build::TreeUpdateBuilder::new();
    // now update the index with content in the working directory for each file
    for file in files {
        // convert this string to a Path
        let rel_path = std::path::Path::new(&file.path);
        let full_path = project_repository.path().join(rel_path);

        // if file exists
        if full_path.exists() {
            // get the blob
            if let Ok(tree_entry) = base_tree.get_path(rel_path) {
                // blob from tree_entry
                let blob = tree_entry
                    .to_object(git_repository)
                    .unwrap()
                    .peel_to_blob()
                    .context("failed to get blob")?;

                // get the contents
                let blob_contents = blob.content();

                let mut patch = "--- original\n+++ modified\n".to_string();

                let mut hunks = file.hunks.to_vec();
                hunks.sort_by_key(|hunk| hunk.start);
                for hunk in hunks {
                    patch.push_str(&hunk.diff);
                }

                // apply patch to blob_contents
                let patch_bytes = patch.as_bytes();
                let patch = Patch::from_bytes(patch_bytes)?;
                let new_content = apply_bytes(blob_contents, &patch)?;

                // create a blob
                let new_blob_oid = git_repository.blob(&new_content)?;
                // upsert into the builder
                builder.upsert(rel_path, new_blob_oid, git2::FileMode::Blob);
            } else {
                // create a git blob from a file on disk
                let blob_oid = git_repository.blob_path(&full_path)?;
                builder.upsert(rel_path, blob_oid, git2::FileMode::Blob);
            }
        } else {
            // remove file from index
            builder.remove(rel_path);
        }
    }

    // now write out the tree
    let tree_oid = builder.create_updated(git_repository, &base_tree)?;
    Ok(tree_oid)
}

fn _print_tree(repo: &git2::Repository, tree: &git2::Tree) -> Result<()> {
    println!("tree id: {:?}", tree.id());
    for entry in tree.iter() {
        println!("  entry: {:?} {:?}", entry.name(), entry.id());
        // get entry contents
        let object = entry.to_object(repo).context("failed to get object")?;
        let blob = object.as_blob().context("failed to get blob")?;
        // convert content to string
        let content =
            std::str::from_utf8(blob.content()).context("failed to convert content to string")?;
        println!("    blob: {:?}", content);
    }
    Ok(())
}

pub fn commit(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
    message: &str,
) -> Result<()> {
    if conflicts::is_conflicting(project_repository, None)? {
        bail!("cannot commit, project is in a conflicted state");
    }

    // get the files to commit
    let statuses = get_status_by_branch(gb_repository, project_repository)
        .context("failed to get status by branch")?;

    for (mut branch, files) in statuses {
        if branch.id == branch_id {
            let tree_oid = write_tree(gb_repository, project_repository, &branch, &files)?;

            let git_repository = &project_repository.git_repository;
            let parent_commit = git_repository.find_commit(branch.head).unwrap();
            let tree = git_repository.find_tree(tree_oid).unwrap();

            // now write a commit, using a merge parent if it exists
            let (author, committer) = gb_repository.git_signatures().unwrap();
            let extra_merge_parent = conflicts::merge_parent(project_repository)?;

            match extra_merge_parent {
                Some(merge_parent) => {
                    let merge_parent = git_repository.find_commit(merge_parent).unwrap();
                    let commit_oid = git_repository
                        .commit(
                            None,
                            &author,
                            &committer,
                            message,
                            &tree,
                            &[&parent_commit, &merge_parent],
                        )
                        .unwrap();
                    branch.head = commit_oid;
                    conflicts::clear(project_repository)?;
                }
                None => {
                    let commit_oid = git_repository.commit(
                        None,
                        &author,
                        &committer,
                        message,
                        &tree,
                        &[&parent_commit],
                    )?;
                    branch.head = commit_oid;
                }
            }

            // update the virtual branch head
            branch.tree = tree_oid;
            let writer = branch::Writer::new(gb_repository);
            writer.write(&branch)?;

            update_gitbutler_integration(gb_repository, project_repository)?;
        }
    }
    Ok(())
}
fn name_to_branch(name: &str) -> String {
    let cleaned_name = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();

    format!("refs/heads/{}", cleaned_name)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    UnsupportedAuthCredentials(git2::CredentialType),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::UnsupportedAuthCredentials(cred_type) => {
                write!(f, "unsupported credential type: {:?}", cred_type)
            }
            err => err.fmt(f),
        }
    }
}

pub fn push(
    project_repository: &project_repository::Repository,
    gb_repository: &gb_repository::Repository,
    branch_id: &str,
) -> Result<(), Error> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")
        .map_err(Error::Other)?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")
        .map_err(Error::Other)?;

    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let mut vbranch = branch_reader
        .read(branch_id)
        .context("failed to read branch")
        .map_err(Error::Other)?;

    let upstream = vbranch
        .upstream
        .unwrap_or_else(|| name_to_branch(&vbranch.name));

    match project_repository.push(&vbranch.head, &upstream) {
        Ok(_) => Ok(()),
        Err(project_repository::Error::UnsupportedAuthCredentials(cred_type)) => {
            return Err(Error::UnsupportedAuthCredentials(cred_type))
        }
        Err(err) => Err(Error::Other(err.into())),
    }?;

    vbranch.upstream = Some(upstream);
    branch_writer
        .write(&vbranch)
        .context("failed to write target branch after push")?;

    project_repository
        .fetch()
        .context("failed to fetch after push")
        .map_err(Error::Other)
}

pub fn get_target_data(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Option<target::Target>> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    match get_default_target(&current_session_reader)? {
        None => Ok(None),
        Some(mut target) => {
            let repo = &project_repository.git_repository;
            let branch = repo.find_branch(&target.branch_name, git2::BranchType::Remote)?;
            let commit = branch.get().peel_to_commit()?;
            let oid = commit.id();
            target.behind = project_repository
                .distance(oid, target.sha)
                .context(format!("failed to get behind for {}", target.branch_name))?;
            Ok(Some(target))
        }
    }
}
