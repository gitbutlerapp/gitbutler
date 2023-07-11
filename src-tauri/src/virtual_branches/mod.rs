pub mod branch;
mod iterator;
pub mod target;

use std::{
    collections::{HashMap, HashSet},
    fmt, path, time, vec,
};

use anyhow::{bail, Context, Result};
use diffy::{apply_bytes, Patch};
use serde::Serialize;

pub use branch::Branch;
pub use iterator::BranchIterator as Iterator;
use uuid::Uuid;

use crate::{gb_repository, project_repository, reader, sessions};

use self::branch::{BranchCreateRequest, FileOwnership, Hunk, Ownership};

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
    pub order: usize,
    pub upstream: String,
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
    pub path: String,
    pub hunks: Vec<VirtualBranchHunk>,
    pub modified_at: u128,
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
    pub name: String,
    pub diff: String,
    pub modified_at: u128,
    pub file_path: String,
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
    sha: String,
    branch: String,
    name: String,
    description: String,
    last_commit_ts: u128,
    first_commit_ts: u128,
    ahead: u32,
    behind: u32,
    upstream: String,
    authors: Vec<String>,
    mergeable: bool,
    merge_conflicts: Vec<String>,
}

fn get_or_choose_default_target(
    gb_repository: &gb_repository::Repository,
    current_session_reader: &sessions::Reader,
    project_repository: &project_repository::Repository,
) -> Result<Option<target::Target>> {
    let target_reader = target::Reader::new(current_session_reader);
    match target_reader.read_default() {
        Ok(target) => Ok(Some(target)),
        Err(reader::Error::NotFound) => {
            match choose_default_branch_name(project_repository)
                .context("failed to choose default remote")?
            {
                None => Ok(None),
                Some(branch_name) => {
                    match gb_repository.set_target_branch(project_repository, &branch_name) {
                        Ok(target) => Ok(Some(target)),
                        Err(e) => {
                            log::error!(
                                "{}: failed to set assumed target branch: {:#}",
                                gb_repository.project_id,
                                e
                            );
                            Ok(None)
                        }
                    }
                }
            }
        }
        Err(e) => Err(e).context("failed to read default target"),
    }
}

fn choose_default_branch_name(
    project_repository: &project_repository::Repository,
) -> Result<Option<String>> {
    let names: HashSet<String> = project_repository
        .git_remote_branches()?
        .iter()
        .cloned()
        .collect();
    let found = names
        .get("origin/master")
        .or(names.get("origin/main"))
        .cloned();
    Ok(found)
}

pub fn apply_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let repo = &project_repository.git_repository;

    let wd_tree = get_wd_tree(repo)?;

    let default_target = match get_or_choose_default_target(
        gb_repository,
        &current_session_reader,
        project_repository,
    )
    .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(()),
    };

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| !branch.applied)
        .collect::<Vec<_>>();

    let writer = branch::Writer::new(gb_repository);

    let mut target_branch = virtual_branches
        .iter()
        .find(|b| b.id == branch_id)
        .context("failed to find target branch")?
        .clone();
    let target_commit = gb_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;
    let target_tree = target_commit.tree().context("failed to get target tree")?;

    let branch_tree = gb_repository
        .git_repository
        .find_tree(target_branch.tree)
        .context("failed to find branch tree")?;

    let merge_options = git2::MergeOptions::new();

    // check index for conflicts
    let mut merge_index = repo
        .merge_trees(&target_tree, &wd_tree, &branch_tree, Some(&merge_options))
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        bail!("conflict applying branch");
    } else {
        // apply the branch
        target_branch.applied = true;
        writer.write(&target_branch)?;

        // checkout the merge index
        let mut checkout_options = git2::build::CheckoutBuilder::new();
        checkout_options.force();
        repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;
    }

    Ok(())
}

// to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
pub fn unapply_branch(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let project = project_repository.project;

    let default_target = match get_or_choose_default_target(
        gb_repository,
        &current_session_reader,
        project_repository,
    )
    .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(()),
    };

    let branch_reader = branch::Reader::new(&current_session_reader);

    let writer = branch::Writer::new(gb_repository);

    let mut target_branch = branch_reader
        .read(branch_id)
        .context("failed to read branch")?;

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
    let target_tree = target_commit.tree().context("failed to get target tree")?;

    if let Ok((branch, files)) = status {
        let tree = write_tree(gb_repository, project_repository, files)?;

        // for each file, go through all the other applied branches and see if they have hunks of the file
        // if so, apply those hunks to the target file and write that back to the working directory
        for file in files {
            let full_path = std::path::Path::new(&project.path).join(&file.path);
            let rel_path = std::path::Path::new(&file.path);

            if let Ok(tree_entry) = target_tree.get_path(std::path::Path::new(rel_path)) {
                // if there is a tree entry in the target, then we can have multiple branches with possible changes

                // blob from tree_entry
                let blob = &tree_entry
                    .to_object(&gb_repository.git_repository)?
                    .peel_to_blob()
                    .expect("failed to get blob");

                // get the base contents
                let blob_contents = blob.content();
                let mut all_hunks = Vec::new();

                // ok, go through all the other branches and find every hunk anywhere else and push them all to `all_hunks`
                for status in &statuses {
                    let (status_branch, status_files) = status;
                    if status_branch.id != branch.id {
                        for status_file in status_files {
                            if status_file.path == file.path {
                                for hunk in &status_file.hunks {
                                    all_hunks.push(hunk);
                                }
                            }
                        }
                    }
                }

                // now order all the hunks by start line and make one patch
                let mut patch = "--- original\n+++ modified\n".to_string();
                // let mut hunks = file.hunks.to_vec();
                // hunks.sort_by_key(|hunk| hunk.start);
                // for hunk in hunks {
                //     patch.push_str(&hunk.diff);
                // }

                let mut all_diffs: Vec<String> = all_hunks
                    .iter()
                    .map(|s| s.diff.clone()) // extract the 'diff' field from each struct
                    .collect(); // collect into a new vector
                all_diffs.sort_by_key(|diff| {
                    let index = all_hunks.iter().position(|s| s.diff == *diff).unwrap(); // unwrap is safe assuming diffs are always found
                    all_hunks[index].start
                });

                for diff in all_diffs {
                    patch.push_str(&diff.clone());
                }

                // apply patch to blob_contents
                let patch_bytes = patch.as_bytes();
                let patch = Patch::from_bytes(patch_bytes)?;
                let new_content = apply_bytes(blob_contents, &patch)?;
                std::fs::write(full_path, new_content)?;
            } else {
                // if there is no file in the target, then we can only have one branch with possible changes, which this branch owns
                // so we can just delete the file
                std::fs::remove_file(full_path)?;
            }
        }
        target_branch.tree = tree;
        target_branch.applied = false;
        writer.write(&target_branch)?;
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

    let default_target = match get_or_choose_default_target(
        gb_repository,
        &current_session_reader,
        project_repository,
    )
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

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| !branch.upstream.is_empty())
        .map(|branch| branch.upstream.replace("refs/heads/", ""))
        .collect::<HashSet<_>>();

    let mut branches: Vec<RemoteBranch> = Vec::new();
    let mut most_recent_branches_by_hash: HashMap<git2::Oid, (git2::Branch, u64)> = HashMap::new();

    for branch in repo.branches(None)? {
        let (branch, _) = branch?;
        match branch.get().target() {
            Some(branch_oid) => {
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

                if virtual_branches.contains(&branch_name) {
                    continue;
                }
                if branch_name == "HEAD" {
                    continue;
                }
                if branch_name == "gitbutler/temp" {
                    continue;
                }

                match most_recent_branches_by_hash.get(&branch_oid) {
                    Some((_, existing_seconds)) => {
                        let branch_name =
                            branch.get().name().context("could not get branch name")?;
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
            None => {
                continue;
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

    for branch in &top_branches {
        let branch_name = branch.get().name().context("could not get branch name")?;
        let upstream_branch = branch.upstream();
        match branch.get().target() {
            Some(branch_oid) => {
                // get the branch ref
                let branch_commit = repo
                    .find_commit(branch_oid)
                    .context("failed to find branch commit")?;

                let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
                revwalk
                    .set_sorting(git2::Sort::TOPOLOGICAL)
                    .context("failed to set sorting")?;
                revwalk.push(main_oid).context("failed to push main oid")?;
                revwalk
                    .hide(branch_oid)
                    .context("failed to hide branch oid")?;

                let mut count_behind = 0;
                for oid in revwalk.flatten() {
                    if oid == branch_oid {
                        break;
                    }
                    count_behind += 1;
                    if count_behind > 100 {
                        break;
                    }
                }

                let mut revwalk2 = repo.revwalk().context("failed to create revwalk")?;
                revwalk2
                    .set_sorting(git2::Sort::TOPOLOGICAL)
                    .context("failed to set sorting")?;
                revwalk2
                    .push(branch_oid)
                    .context("failed to push branch oid")?;
                revwalk2.hide(main_oid).context("failed to hide main oid")?;

                let mut min_time = None;
                let mut max_time = None;
                let mut count_ahead = 0;
                let mut authors = HashSet::new();
                for oid in revwalk2.flatten() {
                    if oid == main_oid {
                        break;
                    }
                    let commit = repo.find_commit(oid).context("failed to find commit")?;
                    let timestamp = commit.time().seconds() as u128;

                    if min_time.is_none() || timestamp < min_time.unwrap() {
                        min_time = Some(timestamp);
                    }

                    if max_time.is_none() || timestamp > max_time.unwrap() {
                        max_time = Some(timestamp);
                    }

                    // find the signature for this commit
                    let commit = repo.find_commit(oid).context("failed to find commit")?;
                    let signature = commit.author();
                    authors.insert(signature.email().unwrap().to_string());

                    count_ahead += 1;
                }

                let upstream_branch_name = match upstream_branch {
                    Ok(upstream_branch) => upstream_branch.get().name().unwrap_or("").to_string(),
                    Err(_) => "".to_string(),
                };

                if count_ahead > 0 {
                    if let Ok(base_tree) = find_base_tree(repo, &branch_commit, &target_commit) {
                        // determine if this tree is mergeable
                        let branch_tree = branch_commit.tree()?;
                        let (mergeable, merge_conflicts) =
                            check_mergeable(repo, &base_tree, &branch_tree, &wd_tree)?;

                        branches.push(RemoteBranch {
                            sha: branch_oid.to_string(),
                            branch: branch_name.to_string(),
                            name: branch_name.to_string(),
                            description: "".to_string(),
                            last_commit_ts: max_time.unwrap_or(0),
                            first_commit_ts: min_time.unwrap_or(0),
                            ahead: count_ahead,
                            behind: count_behind,
                            upstream: upstream_branch_name,
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
                    branch: branch_name.to_string(),
                    name: branch_name.to_string(),
                    description: "".to_string(),
                    last_commit_ts: 0,
                    first_commit_ts: 0,
                    ahead: 0,
                    behind: 0,
                    upstream: "".to_string(),
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
) -> Result<Vec<VirtualBranch>> {
    let mut branches: Vec<VirtualBranch> = Vec::new();
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;

    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session reader")?;

    let default_target = match get_or_choose_default_target(
        gb_repository,
        &current_session_reader,
        project_repository,
    )
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
        if (default_target.sha != branch.head) && branch.applied {
            let vtree = write_tree(gb_repository, project_repository, &files)?;
            let repo = &project_repository.git_repository;
            // get the trees
            let commit_old = repo.find_commit(branch.head)?;
            let tree_old = commit_old.tree()?;
            let vtree_tree = repo.find_tree(vtree)?;

            // do a diff between branch.head and the tree we _would_ commit
            let diff = repo.diff_tree_to_tree(Some(&tree_old), Some(&vtree_tree), None)?;
            let hunks_by_filepath = diff_to_hunks_by_filepath(diff, project_repository)?;

            vfiles = hunks_by_filepath
                .iter()
                .map(|(file_path, hunks)| VirtualBranchFile {
                    id: file_path.clone(),
                    path: file_path.to_string(),
                    hunks: hunks.clone(),
                    modified_at: hunks.iter().map(|h| h.modified_at).max().unwrap_or(0),
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
        if !branch.upstream.is_empty() {
            // get the target remote
            let remote_url = &default_target.remote;
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

                if url == remote_url {
                    upstream_remote = Some(remote);
                    break;
                }
            }
            if let Some(remote) = upstream_remote {
                // remove "refs/heads/" from the branch name
                let branch_name = branch.upstream.replace("refs/heads/", "");
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
            let mut revwalk = repo.revwalk()?;
            revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
            revwalk.push(upstream.id())?;
            // find merge base between upstream and default_target.sha
            let merge_base = repo.merge_base(upstream.id(), default_target.sha)?;
            revwalk.hide(merge_base)?;
            for oid in revwalk.flatten() {
                upstream_commits.insert(oid, true);
            }
        }

        // find all commits on head that are not on target.sha
        let mut commits = vec![];
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
        revwalk.push(branch.head)?;
        revwalk.hide(default_target.sha)?;
        for oid in revwalk {
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            let timestamp = commit.time().seconds() as u128;
            let signature = commit.author();
            let name = signature.name().unwrap().to_string();
            let email = signature.email().unwrap().to_string();
            let message = commit.message().unwrap().to_string();
            let sha = oid.to_string();
            let is_remote = upstream_commits.contains_key(&oid);

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
        if !branch.applied {
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
                    check_mergeable(repo, &base_tree, &branch_tree, &wd_tree)?;
            } else {
                // there is no common base
                mergeable = false;
            };
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
            upstream: branch.upstream.to_string().replace("refs/heads/", ""),
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
    let upstream = "refs/heads/".to_string() + &name;
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let default_target =
        get_or_choose_default_target(gb_repository, &current_session_reader, project_repository)
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
    let diff = repo.diff_tree_to_tree(Some(&merge_tree), Some(&tree), None)?;
    let hunks_by_filepath = diff_to_hunks_by_filepath(diff, project_repository)?;

    // assign ownership to the branch
    for hunk in hunks_by_filepath.values().flatten() {
        branch
            .ownership
            .put(&FileOwnership::try_from(format!("{}:{}", hunk.file_path, hunk.id)).unwrap());
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
        .unwrap_or_else(|| format!("Branch {}", virtual_branches.len() + 1));

    let mut branch = Branch {
        id: Uuid::new_v4().to_string(),
        name,
        applied: true,
        upstream: "".to_string(),
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

fn diff_to_hunks_by_filepath(
    diff: git2::Diff,
    project_repository: &project_repository::Repository,
) -> Result<HashMap<String, Vec<VirtualBranchHunk>>> {
    // find all the hunks
    let mut hunks_by_filepath: HashMap<String, Vec<VirtualBranchHunk>> = HashMap::new();
    let mut current_diff = String::new();

    let mut current_file_path: Option<path::PathBuf> = None;
    let mut current_hunk_id: Option<String> = None;
    let mut current_start: Option<usize> = None;
    let mut current_end: Option<usize> = None;
    let mut mtimes: HashMap<path::PathBuf, u128> = HashMap::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        let file_path = delta.new_file().path().unwrap_or_else(|| {
            delta
                .old_file()
                .path()
                .expect("failed to get file name from diff")
        });

        let (hunk_id, hunk_start, hunk_end) = if let Some(hunk) = hunk {
            (
                format!(
                    "{}-{}",
                    hunk.new_start(),
                    hunk.new_start() + hunk.new_lines()
                ),
                hunk.new_start(),
                hunk.new_start() + hunk.new_lines(),
            )
        } else {
            return true;
        };

        let is_path_changed = if current_file_path.is_none() {
            false
        } else {
            !file_path.eq(current_file_path.as_ref().unwrap())
        };

        let is_hunk_changed = if current_hunk_id.is_none() {
            false
        } else {
            !hunk_id.eq(current_hunk_id.as_ref().unwrap())
        };

        let mtime = get_mtime(
            &mut mtimes,
            &project_repository
                .git_repository
                .workdir()
                .unwrap()
                .join(file_path),
        );
        if is_hunk_changed || is_path_changed {
            let file_path = current_file_path
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            hunks_by_filepath
                .entry(file_path.clone())
                .or_default()
                .push(VirtualBranchHunk {
                    id: current_hunk_id.as_ref().unwrap().to_string(),
                    name: "".to_string(),
                    diff: current_diff.clone(),
                    modified_at: mtime,
                    file_path,
                    start: current_start.unwrap(),
                    end: current_end.unwrap(),
                    hash: diff_hash(current_diff.as_str()),
                });
            current_diff = String::new();
        }

        match line.origin() {
            '+' | '-' | ' ' => current_diff.push_str(&format!("{}", line.origin())),
            _ => {}
        }

        current_diff.push_str(std::str::from_utf8(line.content()).unwrap());
        current_file_path = Some(file_path.to_path_buf());
        current_hunk_id = Some(hunk_id);
        current_start = Some(hunk_start as usize);
        current_end = Some(hunk_end as usize);

        true
    })
    .context("failed to print diff")?;

    if let Some(file_path) = current_file_path {
        let mtime = get_mtime(
            &mut mtimes,
            &project_repository
                .git_repository
                .workdir()
                .unwrap()
                .join(&file_path),
        );
        let file_path = file_path.to_str().unwrap().to_string();
        hunks_by_filepath
            .entry(file_path.clone())
            .or_default()
            .push(VirtualBranchHunk {
                id: current_hunk_id.as_ref().unwrap().to_string(),
                name: "".to_string(),
                modified_at: mtime,
                file_path,
                start: current_start.unwrap(),
                end: current_end.unwrap(),
                hash: diff_hash(current_diff.as_str()),
                diff: current_diff,
            });
    }
    Ok(hunks_by_filepath)
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

    let default_target = match get_or_choose_default_target(
        gb_repository,
        &current_session_reader,
        project_repository,
    ) {
        Ok(Some(target)) => Ok(target),
        Ok(None) => {
            return Ok(vec![]);
        }
        Err(e) => Err(e),
    }
    .context("failed to read default target")?;

    let diff = project_repository
        .workdir_diff(&default_target.sha)
        .context(format!(
            "failed to get diff workdir with {}",
            default_target.sha
        ))?;

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    if virtual_branches.is_empty() {
        // create an empty virtual branch
        virtual_branches =
            vec![
                create_virtual_branch(gb_repository, &BranchCreateRequest::default())
                    .context("failed to default branch")?,
            ];
    }

    // sort by order, so that the default branch is first (left in the ui)
    virtual_branches.sort_by(|a, b| a.order.cmp(&b.order));

    // align branch ownership to the real hunks:
    // - update shifted hunks
    // - remove non existent hunks
    let mut hunks_by_filepath = diff_to_hunks_by_filepath(diff, project_repository)?;

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
                hunk.file_path, hunk.start, hunk.end, hunk.hash, hunk.modified_at,
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
            .fold(HashMap::<String, Vec<_>>::new(), |mut acc, hunk| {
                acc.entry(hunk.file_path.clone())
                    .or_default()
                    .push(hunk.clone());
                acc
            })
            .into_iter()
            .map(|(file_path, hunks)| VirtualBranchFile {
                id: file_path.clone(),
                path: file_path,
                hunks: hunks.clone(),
                modified_at: hunks.iter().map(|h| h.modified_at).max().unwrap_or(0),
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
    let mut target =
        get_or_choose_default_target(gb_repository, &current_session_reader, project_repository)
            .context("failed to get target")?
            .context("no target found")?;

    let repo = &project_repository.git_repository;
    let branch = repo
        .find_branch(&target.name, git2::BranchType::Remote)
        .context(format!("failed to find branch {}", target.name))?;
    let new_target_commit = branch
        .get()
        .peel_to_commit()
        .context(format!("failed to peel branch {} to commit", target.name))?;
    let new_target_oid = new_target_commit.id();
    //
    // if the target has not changed, do nothing
    if new_target_oid == target.sha {
        return Ok(());
    }

    // ok, target has changed, so now we need to merge it into our current work and update our branches
    // first, pull the current state of the working directory into the index
    let wd_tree = get_wd_tree(repo)?;

    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    // get all virtual branches that are applied
    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    let vbranches = list_virtual_branches(gb_repository, project_repository)?;

    let merge_options = git2::MergeOptions::new();

    // get tree from new target
    let new_target_commit = repo.find_commit(new_target_oid)?;
    let new_target_tree = new_target_commit.tree()?;
    // get tree from target.sha
    let target_commit = repo.find_commit(target.sha)?;
    let target_tree = target_commit.tree()?;

    // check index for conflicts
    let mut merge_index = repo
        .merge_trees(
            &target_tree,
            &wd_tree,
            &new_target_tree,
            Some(&merge_options),
        )
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        // TODO: upstream won't merge, so unapply all the vbranches and reset the wd
        bail!("merge conflict");
    }

    // write the currrent target sha to a temp branch as a parent
    let my_ref = "refs/heads/gitbutler/integration";
    repo.reference(my_ref, target.sha, true, "update target")?;
    // get commit object from target.sha
    let target_commit = repo.find_commit(target.sha)?;

    // commit index to temp head for the merge
    repo.set_head(my_ref).context("failed to set head")?;
    let (author, committer) = gb_repository.git_signatures()?;
    let message = "gitbutler joint commit"; // TODO: message that says how to get back to where they were
    repo.commit(
        Some("HEAD"),
        &author,
        &committer,
        message,
        &wd_tree,
        &[&target_commit],
    )?;

    // now we can try to merge the upstream branch into our current working directory
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force();
    repo.checkout_index(Some(&mut merge_index), Some(&mut checkout_options))?;

    // ok, if that worked, then we can try to update all our virtual branches and write out our new target
    let writer = branch::Writer::new(gb_repository);

    // update the heads of all our virtual branches
    for virtual_branch in &mut virtual_branches {
        let mut virtual_branch = virtual_branch.clone();
        // get the matching vbranch
        let vbranch = vbranches
            .iter()
            .find(|vbranch| vbranch.id == virtual_branch.id)
            .unwrap();

        if target.sha == virtual_branch.head {
            // there were no commits, so just update the head
            virtual_branch.head = new_target_oid;
            writer.write(&virtual_branch)?;
        } else {
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
            if merge_index.has_conflicts() {
                // unapply branch for now
                virtual_branch.applied = false;
                writer.write(&virtual_branch)?;
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
                if merge_tree_oid == new_target_tree.id() && vbranch.files.is_empty() {
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

        // write new target oid
        target.sha = new_target_oid;
        let target_writer = target::Writer::new(gb_repository);
        target_writer.write_default(&target)?;
    }

    Ok(())
}

fn write_tree(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    files: &Vec<VirtualBranchFile>,
) -> Result<git2::Oid> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get current session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;
    let default_target =
        get_or_choose_default_target(gb_repository, &current_session_reader, project_repository)
            .context("failed to get target")?
            .context("no target found")?;

    // read the base sha into an index
    let git_repository = &project_repository.git_repository;
    let base_commit = git_repository.find_commit(default_target.sha)?;
    let base_tree = base_commit.tree()?;

    let mut builder = git2::build::TreeUpdateBuilder::new();
    let project = project_repository.project;

    // now update the index with content in the working directory for each file
    for file in files {
        // convert this string to a Path
        let full_path = std::path::Path::new(&project.path).join(&file.path);
        let rel_path = std::path::Path::new(&file.path);

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

pub fn commit(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
    message: &str,
    merge_parent: Option<&git2::Oid>,
) -> Result<()> {
    // get the files to commit
    let statuses = get_status_by_branch(gb_repository, project_repository)
        .context("failed to get status by branch")?;

    for (mut branch, files) in statuses {
        if branch.id == branch_id {
            let tree_oid = write_tree(gb_repository, project_repository, &files)?;

            let git_repository = &project_repository.git_repository;
            let parent_commit = git_repository.find_commit(branch.head).unwrap();
            let tree = git_repository.find_tree(tree_oid).unwrap();

            // now write a commit, using a merge parent if it exists
            let (author, committer) = gb_repository.git_signatures().unwrap();
            match merge_parent {
                Some(merge_parent) => {
                    let merge_parent = git_repository.find_commit(*merge_parent).unwrap();
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
                }
                None => {
                    let commit_oid = git_repository
                        .commit(None, &author, &committer, message, &tree, &[&parent_commit])
                        .unwrap();
                    branch.head = commit_oid;
                }
            }

            // update the virtual branch head
            branch.tree = tree_oid;
            let writer = branch::Writer::new(gb_repository);
            writer.write(&branch)?;
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

    let upstream = if vbranch.upstream.is_empty() {
        name_to_branch(&vbranch.name)
    } else {
        vbranch.upstream.clone()
    };

    match project_repository.push(&vbranch.head, &upstream) {
        Ok(_) => Ok(()),
        Err(project_repository::Error::UnsupportedAuthCredentials(cred_type)) => {
            return Err(Error::UnsupportedAuthCredentials(cred_type))
        }
        Err(err) => Err(Error::Other(err.into())),
    }?;

    vbranch.upstream = upstream;
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
    match get_or_choose_default_target(gb_repository, &current_session_reader, project_repository)?
    {
        None => Ok(None),
        Some(mut target) => {
            let repo = &project_repository.git_repository;
            let branch = repo
                .find_branch(&target.name, git2::BranchType::Remote)
                .unwrap();
            let commit = branch.get().peel_to_commit().unwrap();
            let oid = commit.id();
            target.behind = project_repository
                .behind(oid, target.sha)
                .context(format!("failed to get behind for {}", target.name))?;
            Ok(Some(target))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use tempfile::tempdir;

    use crate::{projects, storage, users};

    use super::*;

    fn commit_all(repository: &git2::Repository) -> Result<git2::Oid> {
        let mut index = repository.index()?;
        index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        let commit_oid = repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "some commit",
            &repository.find_tree(oid)?,
            &[&repository.find_commit(repository.refname_to_id("HEAD")?)?],
        )?;
        Ok(commit_oid)
    }

    fn test_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init(path)?;
        repository.remote_add_fetch("origin/master", "master")?;
        let mut index = repository.index()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid)?,
            &[],
        )?;
        Ok(repository)
    }

    #[test]
    fn test_commit_on_branch_then_change_file_then_get_status() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\nline7\nline8\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\n",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 0);

        // commit
        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "test commit",
            None,
        )?;

        // status (no files)
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 0);
        assert_eq!(branch.commits.len(), 1);

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\nlineBLAH\nline7\nline8\n",
        )?;

        // should have just the last change now, the other line is committed
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 1);

        Ok(())
    }

    #[test]
    fn test_create_branch_with_ownership() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )
        .unwrap();

        let branch0 = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");

        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let current_session = gb_repo.get_or_create_current_session().unwrap();
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session).unwrap();
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch0 = branch_reader.read(&branch0.id).unwrap();

        let branch1 = create_virtual_branch(
            &gb_repo,
            &BranchCreateRequest {
                ownership: Some(branch0.ownership),
                ..Default::default()
            },
        )
        .expect("failed to create virtual branch");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch0.id].len(), 0);
        assert_eq!(files_by_branch_id[&branch1.id].len(), 1);

        Ok(())
    }

    #[test]
    fn test_create_branch_in_the_middle() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");
        create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");
        create_virtual_branch(
            &gb_repo,
            &BranchCreateRequest {
                order: Some(1),
                ..Default::default()
            },
        )
        .expect("failed to create virtual branch");

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

        let mut branches = iterator::BranchIterator::new(&current_session_reader)?
            .collect::<Result<Vec<branch::Branch>, reader::Error>>()
            .expect("failed to read branches");
        branches.sort_by_key(|b| b.order);
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "Branch 1");
        assert_eq!(branches[1].name, "Branch 3");
        assert_eq!(branches[2].name, "Branch 2");

        Ok(())
    }

    #[test]
    fn test_create_branch_no_arguments() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

        let branches = iterator::BranchIterator::new(&current_session_reader)?
            .collect::<Result<Vec<branch::Branch>, reader::Error>>()
            .expect("failed to read branches");
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "Branch 1");
        assert!(branches[0].applied);
        assert_eq!(branches[0].ownership, Ownership::default());
        assert_eq!(branches[0].order, 0);

        Ok(())
    }

    #[test]
    fn test_hunk_expantion() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        // even though selected branch has changed
        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch1_id.clone(),
                order: Some(1),
                ..Default::default()
            },
        )?;
        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch2_id.clone(),
                order: Some(0),
                ..Default::default()
            },
        )?;

        // a slightly different hunk should still go to the same branch
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\n",
        )?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        Ok(())
    }

    #[test]
    fn test_get_status_files_by_branch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        Ok(())
    }

    #[test]
    fn test_updated_ownership_should_bubble_up() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        // write first file
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files = branch_reader.read(&branch1_id)?.ownership.files;
        assert_eq!(files, vec!["test.txt:11-15,1-5".try_into()?]);
        assert_eq!(
            files[0].hunks[0].timestam_ms(),
            files[0].hunks[1].timestam_ms(),
            "timestamps must be the same"
        );

        thread::sleep(Duration::from_millis(10)); // make sure timestamps are different

        // wriging a new file should put it on the top
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "hello",
        )?;

        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files1 = branch_reader.read(&branch1_id)?.ownership.files;
        assert_eq!(
            files1,
            vec![
                "test2.txt:1-2".try_into()?,
                "test.txt:11-15,1-5".try_into()?
            ]
        );

        assert_ne!(
            files1[0].hunks[0].timestam_ms(),
            files1[1].hunks[0].timestam_ms(),
            "new file timestamp must be different"
        );

        assert_eq!(
            files[0].hunks[0].timestam_ms(),
            files1[1].hunks[0].timestam_ms(),
            "old file timestamp must not change"
        );

        thread::sleep(Duration::from_millis(10)); // make sure timestamps are different

        // update second hunk, it should make both file and hunk bubble up
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1update\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
        get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files2 = branch_reader.read(&branch1_id)?.ownership.files;
        assert_eq!(
            files2,
            vec![
                "test.txt:1-6,11-15".try_into()?,
                "test2.txt:1-2".try_into()?,
            ]
        );

        assert_ne!(
            files2[0].hunks[0].timestam_ms(),
            files2[0].hunks[1].timestam_ms(),
            "new file timestamps must be different"
        );
        assert_eq!(
            files2[0].hunks[1].timestam_ms(),
            files1[1].hunks[0].timestam_ms(),
            "old file timestamp must not change"
        );
        assert_eq!(
            files2[1].hunks[0].timestam_ms(),
            files1[0].hunks[0].timestam_ms(),
            "old file timestamp must not change"
        );

        Ok(())
    }

    #[test]
    fn test_move_hunks_multiple_sources() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch3_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch_writer = branch::Writer::new(&gb_repo);
        let branch2 = branch_reader.read(&branch2_id)?;
        branch_writer.write(&branch::Branch {
            ownership: Ownership {
                files: vec!["test.txt:1-5".try_into()?],
            },
            ..branch2
        })?;
        let branch1 = branch_reader.read(&branch1_id)?;
        branch_writer.write(&branch::Branch {
            ownership: Ownership {
                files: vec!["test.txt:11-15".try_into()?],
            },
            ..branch1
        })?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 3);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id][0].hunks.len(), 1);
        assert_eq!(files_by_branch_id[&branch3_id].len(), 0);

        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch3_id.clone(),
                ownership: Some(Ownership::try_from("test.txt:1-5,11-15")?),
                ..Default::default()
            },
        )?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 3);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch3_id][0].hunks.len(), 2);

        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(branch_reader.read(&branch1_id)?.ownership.files, vec![]);
        assert_eq!(branch_reader.read(&branch2_id)?.ownership.files, vec![]);
        assert_eq!(
            branch_reader.read(&branch3_id)?.ownership.files,
            vec!["test.txt:1-5,11-15".try_into()?]
        );

        Ok(())
    }

    #[test]
    fn test_move_hunks_partial_explicitly() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;

        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 2);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch2_id.clone(),
                ownership: Some(Ownership::try_from("test.txt:1-5")?),
                ..Default::default()
            },
        )?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch1_id][0].hunks.len(), 1);

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(
            branch_reader.read(&branch1_id)?.ownership.files,
            vec!["test.txt:12-16".try_into()?]
        );
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership.files,
            vec!["test.txt:1-5".try_into()?]
        );

        Ok(())
    }

    #[test]
    fn test_add_new_hunk_to_the_end() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;

        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

        create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        assert_eq!(statuses[0].1[0].hunks[0].id, "12-16");

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
        )?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        assert!(statuses[0].1[0].hunks[0]
            .id
            .starts_with("13-17-ad6f6af93b494f66d4754e4806c7c1b4-"));
        assert_eq!(statuses[0].1[0].hunks[1].id, "1-5");

        Ok(())
    }

    #[test]
    fn test_update_branch_target() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\nline7\nline8\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        // create a vbranch
        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        // add a commit to the target branch it's pointing to so there is something "upstream"
        commit_all(&repository)?;
        let up_target = repository.head().unwrap().target().unwrap();

        // revert content
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;

        //update repo ref refs/remotes/origin/master to up_target oid
        repository.reference(
            "refs/remotes/origin/master",
            up_target,
            true,
            "update target",
        )?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\nline7\nline8\nlocal\n",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "test commit",
            None,
        )?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\nline7\nline8\nlocal\nmore local\n",
        )?;

        // add something to the branch
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 1);

        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(String::from_utf8(contents)?, "line1\nline2\nline3\nline4\n");

        // update the target branch
        // this should leave the work on file2, but update the contents of file1
        // and the branch diff should only be on file2
        update_branch_target(&gb_repo, &project_repository)?;

        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(
            String::from_utf8(contents)?,
            "line1\nline2\nline3\nline4\nupstream\n"
        );

        // assert that the vbranch target is updated
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 2); // branch commit, merge commit

        Ok(())
    }

    #[test]
    fn test_update_branch_target_detect_integrated_branches() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        // create a vbranch
        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        // add a commit to the target branch it's pointing to so there is something "upstream"
        commit_all(&repository)?;
        let up_target = repository.head().unwrap().target().unwrap();

        //update repo ref refs/remotes/origin/master to up_target oid
        repository.reference(
            "refs/remotes/origin/master",
            up_target,
            true,
            "update target",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "test commit",
            None,
        )?;

        // add something to the branch
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 0);
        assert_eq!(branch.commits.len(), 1);

        // update the target branch
        // this should notice that the trees are the same after the merge, so it should unapply the branch
        update_branch_target(&gb_repo, &project_repository)?;

        // integrated branch should be deleted
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        assert!(!branches.iter().any(|b| b.id == branch1_id));

        Ok(())
    }

    #[test]
    fn test_update_branch_target_detect_integrated_branches_with_more_work() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        // create a vbranch
        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        // add a commit to the target branch it's pointing to so there is something "upstream"
        commit_all(&repository)?;
        let up_target = repository.head().unwrap().target().unwrap();

        //update repo ref refs/remotes/origin/master to up_target oid
        repository.reference(
            "refs/remotes/origin/master",
            up_target,
            true,
            "update target",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "test commit",
            None,
        )?;

        // add some uncommitted work
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "local\nline1\nline2\nline3\nline4\nupstream\n",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 1);

        // update the target branch
        // this should notice that the trees are the same after the merge, but there are files on the branch, so do a merge and then leave the files there
        update_branch_target(&gb_repo, &project_repository)?;

        // there should be a new vbranch created, but nothing is on it
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 2);

        Ok(())
    }

    #[test]
    fn test_apply_unapply_branch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nbranch1\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line5\nline6\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch2_id,
                ownership: Some(Ownership::try_from("test2.txt:1-3")?),
                ..Default::default()
            },
        )?;

        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(
            "line1\nline2\nline3\nline4\nbranch1\n",
            String::from_utf8(contents)?
        );
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch.files.len(), 1);
        assert!(branch.active);

        unapply_branch(&gb_repo, &project_repository, &branch1_id)?;

        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch.files.len(), 0);
        assert!(!branch.active);

        apply_branch(&gb_repo, &project_repository, &branch1_id)?;
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(
            "line1\nline2\nline3\nline4\nbranch1\n",
            String::from_utf8(contents)?
        );
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch.files.len(), 1);
        assert!(branch.active);

        Ok(())
    }

    #[test]
    fn test_apply_unapply_added_deleted_files() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "file1\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        // rm file_path2, add file3
        std::fs::remove_file(std::path::Path::new(&project.path).join(file_path2))?;
        let file_path3 = std::path::Path::new("test3.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path3),
            "file3\n",
        )?;

        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch3_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch2_id.clone(),
                ownership: Some(Ownership::try_from("test2.txt:0-0")?),
                ..Default::default()
            },
        )?;
        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch3_id.clone(),
                ownership: Some(Ownership::try_from("test3.txt:1-2")?),
                ..Default::default()
            },
        )?;

        unapply_branch(&gb_repo, &project_repository, &branch2_id)?;
        // check that file2 is back
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!("file2\n", String::from_utf8(contents)?);

        unapply_branch(&gb_repo, &project_repository, &branch3_id)?;
        // check that file3 is gone
        assert!(!std::path::Path::new(&project.path)
            .join(file_path3)
            .exists());

        apply_branch(&gb_repo, &project_repository, &branch2_id)?;
        // check that file2 is gone
        assert!(!std::path::Path::new(&project.path)
            .join(file_path2)
            .exists());

        apply_branch(&gb_repo, &project_repository, &branch3_id)?;
        // check that file3 is back
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path3))?;
        assert_eq!("file3\n", String::from_utf8(contents)?);

        Ok(())
    }

    #[test]
    fn test_detect_mergeable_branch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nbranch1\n",
        )?;
        let file_path4 = std::path::Path::new("test4.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path4),
            "line5\nline6\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch_writer = branch::Writer::new(&gb_repo);

        update_branch(
            &gb_repo,
            branch::BranchUpdateRequest {
                id: branch2_id.clone(),
                ownership: Some("test4.txt:1-3".try_into()?),
                ..Default::default()
            },
        )
        .expect("failed to update branch");

        // unapply both branches and create some conflicting ones
        unapply_branch(&gb_repo, &project_repository, &branch1_id)?;
        unapply_branch(&gb_repo, &project_repository, &branch2_id)?;

        // create an upstream remote conflicting commit
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        commit_all(&repository)?;
        let up_target = repository.head().unwrap().target().unwrap();
        repository.reference(
            "refs/remotes/origin/remote_branch",
            up_target,
            true,
            "update target",
        )?;

        // revert content and write a mergeable branch
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        let file_path3 = std::path::Path::new("test3.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path3),
            "file3\n",
        )?;
        commit_all(&repository)?;
        let up_target = repository.head().unwrap().target().unwrap();
        repository.reference(
            "refs/remotes/origin/remote_branch2",
            up_target,
            true,
            "update target",
        )?;
        // remove file_path3
        std::fs::remove_file(std::path::Path::new(&project.path).join(file_path3))?;

        // create branches that conflict with our earlier branches
        create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch");
        let branch4_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        // branch3 conflicts with branch1 and remote_branch
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nbranch3\n",
        )?;

        // branch4 conflicts with branch2
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "line1\nline2\nline3\nline4\nbranch4\n",
        )?;

        let branch4 = branch_reader.read(&branch4_id)?;
        branch_writer.write(&Branch {
            ownership: Ownership {
                files: vec!["test2.txt:1-6".try_into()?],
            },
            ..branch4
        })?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        assert_eq!(branches.len(), 4);

        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert!(!branch1.active);
        assert!(!branch1.mergeable);
        assert_eq!(branch1.merge_conflicts.len(), 1);
        assert_eq!(branch1.merge_conflicts.first().unwrap(), "test.txt");

        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert!(!branch2.active);
        assert!(branch2.mergeable);
        assert_eq!(branch2.merge_conflicts.len(), 0);

        let remotes = remote_branches(&gb_repo, &project_repository)?;
        let remote1 = &remotes
            .iter()
            .find(|b| b.branch == "refs/remotes/origin/remote_branch")
            .unwrap();
        assert!(!remote1.mergeable);
        assert_eq!(remote1.ahead, 1);
        assert_eq!(remote1.merge_conflicts.len(), 1);
        assert_eq!(remote1.merge_conflicts.first().unwrap(), "test.txt");

        let remote2 = &remotes
            .iter()
            .find(|b| b.branch == "refs/remotes/origin/remote_branch2")
            .unwrap();
        assert!(remote2.mergeable);
        assert_eq!(remote2.ahead, 2);
        assert_eq!(remote2.merge_conflicts.len(), 0);

        Ok(())
    }

    #[test]
    fn test_detect_remote_commits() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;
        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch_writer = branch::Writer::new(&gb_repo);

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "http://origin.com/project".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let repo = &project_repository.git_repository;
        repo.remote("origin", "http://origin.com/project")?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        // create a commit to push upstream
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "upstream commit 1",
            None,
        )?;

        // create another commit to push upstream
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\nmore upstream\n",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "upstream commit 2",
            None,
        )?;

        // push the commit upstream
        let branch1 = branch_reader.read(&branch1_id)?;
        let up_target = branch1.head;
        repository.reference(
            "refs/remotes/origin/remote_branch",
            up_target,
            true,
            "update target",
        )?;
        // set the upstream reference
        branch_writer.write(&Branch {
            upstream: "remote_branch".to_string(),
            ..branch1
        })?;

        // create another commit that is not pushed up
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\nmore upstream\nmore work\n",
        )?;

        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "local commit",
            None,
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        assert_eq!(branches.len(), 1);

        let branch = &branches.first().unwrap();
        assert_eq!(branch.commits.len(), 3);
        assert_eq!(branch.commits[0].description, "local commit");
        assert!(!branch.commits[0].is_remote);
        assert_eq!(branch.commits[1].description, "upstream commit 2");
        assert!(branch.commits[1].is_remote);
        assert!(branch.commits[2].is_remote);

        Ok(())
    }

    #[test]
    fn test_create_vbranch_from_remote_branch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "http://origin.com/project".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;
        repository.remote("origin", "http://origin.com/project")?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nbranch\n",
        )?;
        commit_all(&repository)?;
        repository.reference(
            "refs/remotes/branch1",
            repository.head().unwrap().target().unwrap(),
            true,
            "update target",
        )?;

        // reset the first file
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;

        // create a default branch. there should not be anything on this
        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].files.len(), 0);

        // create a new virtual branch from the remote branch
        let branch2_id = create_virtual_branch_from_branch(
            &gb_repo,
            &project_repository,
            "refs/remotes/branch1",
        )?;

        // shouldn't be anything on either of our branches
        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch1.files.len(), 0);
        assert!(branch1.active);
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert_eq!(branch2.files.len(), 0);
        assert!(!branch2.active);

        // file should still be the original
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);

        // this should bring in the branch change
        apply_branch(&gb_repo, &project_repository, &branch2_id)?;

        // file should be the branch version now
        let contents = std::fs::read(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(
            "line1\nline2\nline3\nline4\nbranch\n",
            String::from_utf8(contents)?
        );

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch1.files.len(), 0);
        assert!(branch1.active);
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert_eq!(branch2.files.len(), 0);
        assert!(branch2.active);
        assert_eq!(branch2.commits.len(), 1);

        // add to the applied file in the same hunk so it adds to the second branch
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nbranch\nmore branch\n",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch1.files.len(), 0);
        assert!(branch1.active);
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert_eq!(branch2.files.len(), 1);
        assert!(branch2.active);

        // add to another file so it goes to the default one
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\n",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch1.files.len(), 1);
        assert!(branch1.active);
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert_eq!(branch2.files.len(), 1);
        assert!(branch2.active);

        Ok(())
    }

    #[test]
    fn test_create_vbranch_from_behind_remote_branch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        // create a commit and set the target
        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\n",
        )?;
        commit_all(&repository)?;

        let base_commit = repository.head().unwrap().target().unwrap();

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        commit_all(&repository)?;

        let upstream_commit = repository.head().unwrap().target().unwrap();
        repository.reference(
            "refs/remotes/origin/master",
            upstream_commit,
            true,
            "update target",
        )?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin/master".to_string(),
            remote: "http://origin.com/project".to_string(),
            sha: upstream_commit,
            behind: 0,
        })?;
        repository.remote("origin", "http://origin.com/project")?;

        // reset master to the base commit
        repository.reference("refs/heads/master", base_commit, true, "update target")?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\n",
        )?;
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\nremote",
        )?;
        commit_all(&repository)?;
        let remote_commit = repository.head().unwrap().target().unwrap();

        repository.reference(
            "refs/remotes/origin/branch1",
            remote_commit,
            true,
            "update target",
        )?;

        // reset wd
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nupstream\n",
        )?;
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\n",
        )?;

        // create a new virtual branch from the remote branch
        let branch1_id = create_virtual_branch_from_branch(
            &gb_repo,
            &project_repository,
            "refs/remotes/origin/branch1",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branch1.files.len(), 0);

        // nothing has changed
        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!(contents, "file2\n");

        apply_branch(&gb_repo, &project_repository, &branch1_id)?;

        // the file2 has been updated
        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(contents, "line1\nline2\nline3\nline4\nupstream\n");
        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path2))?;
        assert_eq!(contents, "file2\nremote");

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        // a default branch has been created
        assert_eq!(branches.len(), 2);
        // our branch still no hunks
        assert_eq!(branch1.files.len(), 0);
        assert_eq!(branch1.commits.len(), 2); // a merge commit too

        Ok(())
    }

    #[test]
    fn test_partial_commit() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
        )?;
        commit_all(&repository)?;

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
            behind: 0,
        })?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;
        let branch2_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        // create a change with two hunks
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n",
        )?;

        // move hunk1 and hunk3 to branch2
        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch_writer = branch::Writer::new(&gb_repo);
        let branch2 = branch_reader.read(&branch2_id)?;
        branch_writer.write(&branch::Branch {
            ownership: Ownership {
                files: vec!["test.txt:9-16".try_into()?],
            },
            ..branch2
        })?;
        let branch1 = branch_reader.read(&branch1_id)?;
        branch_writer.write(&branch::Branch {
            ownership: Ownership {
                files: vec!["test.txt:1-6".try_into()?, "test.txt:17-24".try_into()?],
            },
            ..branch1
        })?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        assert_eq!(branch1.files[0].hunks.len(), 2);
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();
        assert_eq!(branch2.files[0].hunks.len(), 1);

        // commit
        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "branch1 commit",
            None,
        )?;
        commit(
            &gb_repo,
            &project_repository,
            &branch2_id,
            "branch2 commit",
            None,
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();
        let branch2 = &branches.iter().find(|b| b.id == branch2_id).unwrap();

        // branch one test.txt has just the 1st and 3rd hunks applied
        let commit = &branch1.commits[0].id;
        let contents = commit_sha_to_contents(&repository, commit, "test.txt");
        assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

        // branch two test.txt has just the middle hunk applied
        let commit = &branch2.commits[0].id;
        let contents = commit_sha_to_contents(&repository, commit, "test.txt");
        assert_eq!(contents, "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n");

        // ok, now we're going to unapply branch1, which should remove the 1st and 3rd hunks
        unapply_branch(&gb_repo, &project_repository, &branch1_id)?;
        // read contents of test.txt
        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(contents, "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n");

        // ok, now we're going to re-apply branch1, which adds hunk 1 and 3, then unapply branch2, which should remove the middle hunk
        apply_branch(&gb_repo, &project_repository, &branch1_id)?;
        unapply_branch(&gb_repo, &project_repository, &branch2_id)?;

        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

        // finally, reapply the middle hunk on branch2, so we have all of them again
        apply_branch(&gb_repo, &project_repository, &branch2_id)?;

        let contents =
            std::fs::read_to_string(std::path::Path::new(&project.path).join(file_path))?;
        assert_eq!(contents, "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\npatch2\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nmiddle\nline11\nline12\npatch3\n");

        Ok(())
    }

    fn commit_sha_to_contents(repository: &git2::Repository, commit: &str, path: &str) -> String {
        let commit = git2::Oid::from_str(commit).expect("failed to parse oid");
        let commit = repository
            .find_commit(commit)
            .expect("failed to get commit object");
        // get the tree
        let tree = commit.tree().expect("failed to get tree");
        // get the blob
        let tree_entry = tree
            .get_path(std::path::Path::new(path))
            .expect("failed to get blob");
        // blob from tree_entry
        let blob = tree_entry
            .to_object(repository)
            .unwrap()
            .peel_to_blob()
            .expect("failed to get blob");

        // get the contents
        let contents = blob.content();
        let contents = std::str::from_utf8(contents).expect("failed to convert to string");
        contents.to_string()
    }

    #[test]
    fn test_commit_add_and_delete_files() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            gb_repo_path,
            project.id.clone(),
            project_store,
            user_store,
        )?;
        let project_repository = project_repository::Repository::open(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "file1\n",
        )?;
        let file_path2 = std::path::Path::new("test2.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path2),
            "file2\n",
        )?;
        commit_all(&repository)?;

        let commit1_oid = repository.head().unwrap().target().unwrap();
        let commit1 = repository.find_commit(commit1_oid).unwrap();
        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "origin".to_string(),
            remote: "origin".to_string(),
            sha: commit1_oid,
            behind: 0,
        })?;

        // remove file
        std::fs::remove_file(std::path::Path::new(&project.path).join(file_path2))?;
        // add new file
        let file_path3 = std::path::Path::new("test3.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path3),
            "file3\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, &BranchCreateRequest::default())
            .expect("failed to create virtual branch")
            .id;

        // commit
        commit(
            &gb_repo,
            &project_repository,
            &branch1_id,
            "branch1 commit",
            None,
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch1 = &branches.iter().find(|b| b.id == branch1_id).unwrap();

        // branch one test.txt has just the 1st and 3rd hunks applied
        let commit2 = &branch1.commits[0].id;
        let commit2 = git2::Oid::from_str(commit2).expect("failed to parse oid");
        let commit2 = repository
            .find_commit(commit2)
            .expect("failed to get commit object");

        let tree = commit1.tree().expect("failed to get tree");
        let file_list = tree_to_file_list(&repository, &tree).unwrap();
        assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

        // get the tree
        let tree = commit2.tree().expect("failed to get tree");
        let file_list = tree_to_file_list(&repository, &tree).unwrap();
        assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

        Ok(())
    }

    fn tree_to_file_list(repository: &git2::Repository, tree: &git2::Tree) -> Result<Vec<String>> {
        let mut file_list = Vec::new();
        for entry in tree.iter() {
            let path = entry.name().unwrap();
            let entry = tree
                .get_path(std::path::Path::new(path))
                .context(format!("failed to get tree entry for path {}", path))?;
            let object = entry
                .to_object(repository)
                .context(format!("failed to get object for tree entry {}", path))?;
            if object.kind() == Some(git2::ObjectType::Blob) {
                file_list.push(path.to_string());
            }
        }
        Ok(file_list)
    }
}
