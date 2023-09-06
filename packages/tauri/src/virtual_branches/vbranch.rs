use std::{
    collections::{HashMap, HashSet},
    os::unix::fs::PermissionsExt,
    path, time, vec,
};

use anyhow::{bail, Context, Result};
use diffy::{apply_bytes, Patch};
use serde::Serialize;

use uuid::Uuid;

use crate::{
    dedup::{dedup, dedup_fmt},
    gb_repository,
    git::{self, diff},
    keys::Key,
    project_repository::{self, conflicts, LogUntil},
    reader, sessions,
};

use super::{
    branch::{self, Branch, BranchCreateRequest, FileOwnership, Hunk, Ownership},
    target, Iterator,
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
    pub notes: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
    pub commits: Vec<VirtualBranchCommit>,
    pub mergeable: bool, // this branch will merge cleanly into the current working directory (only for unapplied branches)
    pub merge_conflicts: Vec<String>, // if mergeable is false, this will contain a list of files that have merge conflicts (only for unapplied branches)
    pub conflicted: bool, // is this branch currently in a conflicted state (only for applied branches)
    pub order: usize,     // the order in which this branch should be displayed in the UI
    pub upstream: Option<git::RemoteBranchName>, // the name of the upstream branch this branch this pushes to
    pub base_current: bool, // is this vbranch based on the current base branch? if false, this needs to be manually merged with conflicts
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
    pub author: Author,
    pub is_remote: bool,
    // only present if is_remote is false
    pub files: Vec<VirtualBranchFile>,
    pub is_integrated: bool,
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
    pub binary: bool,
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
    pub binary: bool,
    pub locked: bool,
}

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
    pub last_commit_ts: u128,
    pub first_commit_ts: u128,
    pub ahead: u32,
    pub behind: u32,
    pub upstream: Option<git::RemoteBranchName>,
    pub authors: Vec<Author>,
    pub mergeable: bool,
    pub merge_conflicts: Vec<String>,
    pub commits: Vec<VirtualBranchCommit>,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub base_sha: String,
    pub current_sha: String,
    pub behind: u32,
    pub upstream_commits: Vec<VirtualBranchCommit>,
    pub recent_commits: Vec<VirtualBranchCommit>,
}

#[derive(Debug, Serialize, Hash, Clone, PartialEq, Eq)]
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

pub fn get_default_target(
    current_session_reader: &sessions::Reader,
) -> Result<Option<target::Target>> {
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

    let writer = branch::Writer::new(gb_repository);

    let mut apply_branch = branch::Reader::new(&current_session_reader)
        .read(branch_id)
        .context(format!("failed to read branch {}", branch_id))?;

    let target_commit = repo
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;
    let target_tree = target_commit.tree().context("failed to get target tree")?;

    let mut branch_tree = repo
        .find_tree(apply_branch.tree)
        .context("failed to find branch tree")?;

    // calculate the merge base and make sure it's the same as the target commit
    // if not, we need to merge or rebase the branch to get it up to date

    let merge_base = repo.merge_base(default_target.sha, apply_branch.head)?;
    if merge_base != default_target.sha {
        // Branch is out of date, merge or rebase it
        let merge_base_tree = repo.find_commit(merge_base)?.tree()?;
        let mut merge_index = repo
            .merge_trees(&merge_base_tree, &branch_tree, &target_tree)
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
        .merge_trees(&target_tree, &wd_tree, &branch_tree)
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

    super::integration::update_gitbutler_integration(gb_repository, project_repository)?;

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

    let applied_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    let applied_statuses = get_applied_status(
        gb_repository,
        project_repository,
        &default_target,
        applied_branches,
    )
    .context("failed to get status by branch")?;

    let status = applied_statuses
        .iter()
        .find(|(s, _)| s.id == branch_id)
        .context("failed to find status for branch");

    let target_commit = gb_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let repo = &project_repository.git_repository;

    if let Ok((_, files)) = status {
        let tree = write_tree(project_repository, &default_target, files)?;

        target_branch.tree = tree;
        target_branch.applied = false;
        branch_writer.write(&target_branch)?;
    }

    // ok, update the wd with the union of the rest of the branches
    let base_tree = target_commit.tree()?;
    let mut final_tree = target_commit.tree()?;

    // go through the other applied branches and merge them into the final tree
    // then check that out into the working directory
    for (branch, files) in applied_statuses {
        if branch.id != branch_id {
            let tree_oid = write_tree(project_repository, &default_target, &files)?;
            let branch_tree = repo.find_tree(tree_oid)?;
            if let Ok(mut result) = repo.merge_trees(&base_tree, &final_tree, &branch_tree) {
                let final_tree_oid = result.write_tree_to(repo)?;
                final_tree = repo.find_tree(final_tree_oid)?;
            }
        }
    }
    // convert the final tree into an object
    let final_tree_oid = final_tree.id();
    let final_tree = repo.find_tree(final_tree_oid)?;

    // checkout final_tree into the working directory
    let mut checkout_options = git2::build::CheckoutBuilder::new();
    checkout_options.force();
    checkout_options.remove_untracked(true);
    repo.checkout_tree(&final_tree, Some(&mut checkout_options))?;

    super::integration::update_gitbutler_integration(gb_repository, project_repository)?;

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
    let target_commit = repo
        .find_commit(main_oid)
        .context("failed to find target commit")?;

    let wd_tree = get_wd_tree(repo)?;

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
        let branch_name = branch.refname().context("could not get branch name")?;
        match branch.target() {
            Some(branch_oid) => {
                // get the branch ref
                let branch_commit = repo
                    .find_commit(branch_oid)
                    .context("failed to find branch commit")?;

                let count_behind = project_repository
                    .distance(main_oid, branch_oid)
                    .context("failed to get behind count")?;

                let ahead = project_repository
                    .log(branch_oid, LogUntil::Commit(main_oid))
                    .context("failed to get ahead commits")?;
                let count_ahead = ahead.len();

                let min_time = ahead.iter().map(|commit| commit.time().seconds()).min();
                let max_time = ahead.iter().map(|commit| commit.time().seconds()).max();
                let authors = ahead
                    .iter()
                    .map(|commit| commit.author())
                    .map(Author::from)
                    .collect::<HashSet<_>>();

                let upstream = branch
                    .upstream()
                    .ok()
                    .map(|upstream_branch| git::RemoteBranchName::try_from(&upstream_branch))
                    .transpose()?;

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
                            commits: ahead
                                .into_iter()
                                .map(|commit| {
                                    commit_to_vbranch_commit(
                                        project_repository,
                                        &default_target,
                                        &commit,
                                        None,
                                    )
                                })
                                .collect::<Result<Vec<_>>>()?,
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
                    commits: vec![],
                });
            }
        }
    }
    Ok(branches)
}

pub fn get_wd_tree(repo: &git::Repository) -> Result<git::Tree> {
    let mut index = repo.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    Ok(tree)
}

fn find_base_tree<'a>(
    repo: &'a git::Repository,
    branch_commit: &'a git::Commit<'a>,
    target_commit: &'a git::Commit<'a>,
) -> Result<git::Tree<'a>> {
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
    Ok(base_tree)
}

fn check_mergeable(
    repo: &git::Repository,
    base_tree: &git::Tree,
    branch_tree: &git::Tree,
    wd_tree: &git::Tree,
) -> Result<(bool, Vec<String>)> {
    let mut merge_conflicts = Vec::new();

    let merge_index = repo
        .merge_trees(base_tree, wd_tree, branch_tree)
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

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to get default target")?
    {
        Some(target) => target,
        None => return Ok(vec![]),
    };

    let wd_tree = get_wd_tree(&project_repository.git_repository)?;

    let statuses = get_status_by_branch(gb_repository, project_repository)?;
    let conflicting_files = conflicts::conflicting_files(project_repository)?;
    for (branch, files) in &statuses {
        let file_hunks = files
            .iter()
            .cloned()
            .map(|file| (file.path, file.hunks))
            .collect::<HashMap<_, _>>();

        let file_hunk_hashes = file_hunks
            .iter()
            .map(|(path, hunks)| {
                (
                    path,
                    hunks
                        .iter()
                        .map(|hunk| hunk.hash.clone())
                        .collect::<HashSet<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();

        // check if head tree does not match target tree
        // if so, we diff the head tree and the new write_tree output to see what is new and filter the hunks to just those
        //
        // TODO: refactor this to instead have branch.commits[].files[] structure
        let vfiles = if default_target.sha != branch.head {
            let vtree = write_tree(project_repository, &default_target, files)?;
            // get the trees
            let tree_old = project_repository
                .git_repository
                .find_commit(branch.head)?
                .tree()?;
            let vtree_tree = project_repository.git_repository.find_tree(vtree)?;

            // do a diff between branch.head and the tree we _would_ commit
            let diff = diff::trees(&project_repository.git_repository, &tree_old, &vtree_tree)
                .context("failed to diff trees")?;

            let non_commited_hunks_by_filepath =
                super::hunks_by_filepath(project_repository, &diff);

            let mut vfiles = non_commited_hunks_by_filepath
                .into_iter()
                .map(|(file_path, mut non_commited_hunks)| {
                    // sort non commited hunks the same way as the real hunks are sorted
                    non_commited_hunks.sort_by_key(|h| {
                        file_hunks
                            .get(&file_path)
                            .map(|hunks| {
                                hunks.iter().position(|h2| {
                                    let h_range = [h.start..=h.end];
                                    let h2_range = [h2.start..=h2.end];
                                    h2_range.iter().any(|line| h_range.contains(line))
                                })
                            })
                            .unwrap_or(Some(0))
                    });

                    let mut conflicted = false;
                    if let Some(conflicts) = &conflicting_files {
                        if conflicts.contains(&file_path.display().to_string()) {
                            // check file for conflict markers, resolve the file if there are none in any hunk
                            for hunk in &non_commited_hunks {
                                if hunk.diff.contains("<<<<<<< ours") {
                                    conflicted = true;
                                }
                                if hunk.diff.contains(">>>>>>> theirs") {
                                    conflicted = true;
                                }
                            }
                            if !conflicted {
                                conflicts::resolve(
                                    project_repository,
                                    &file_path.display().to_string(),
                                )
                                .unwrap();
                            }
                        }
                    }

                    VirtualBranchFile {
                        id: file_path.display().to_string(),
                        path: file_path.clone(),
                        binary: non_commited_hunks.iter().any(|h| h.binary),
                        modified_at: non_commited_hunks
                            .iter()
                            .map(|h| h.modified_at)
                            .max()
                            .unwrap_or(0),
                        hunks: non_commited_hunks
                            .into_iter()
                            .map(|hunk| VirtualBranchHunk {
                                // we consider a hunk to be locked if it's not seen verbatim
                                // non-commited. reason beging - we can't partialy move hunks between
                                // branches just yet.
                                locked: file_hunk_hashes
                                    .get(&file_path)
                                    .map(|h| !h.contains(&hunk.hash))
                                    .unwrap_or(false),
                                ..hunk
                            })
                            .collect::<Vec<_>>(),
                        conflicted,
                    }
                })
                .collect::<Vec<_>>();

            // stable files sort using virtual files position
            vfiles.sort_by_key(|a| files.iter().position(|f| f.id == a.id).unwrap_or(0));

            vfiles
        } else {
            files.to_vec()
        };

        let repo = &project_repository.git_repository;

        let branch_commit = repo
            .find_commit(branch.head)
            .context("failed to find branch commit")?;

        // see if we can identify some upstream
        let mut upstream_commit = None;
        if let Some(branch_upstream) = &branch.upstream {
            if let Ok(upstream_oid) = repo.refname_to_id(&branch_upstream.to_string()) {
                if let Ok(upstream_commit_obj) = repo.find_commit(upstream_oid) {
                    upstream_commit = Some(upstream_commit_obj);
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
            for oid in project_repository.l(upstream.id(), LogUntil::Commit(merge_base))? {
                upstream_commits.insert(oid, true);
            }
        }

        // find all commits on head that are not on target.sha
        let mut commits = vec![];
        for commit in project_repository
            .log(branch.head, LogUntil::Commit(default_target.sha))
            .context(format!("failed to get log for branch {}", branch.name))?
        {
            let commit = commit_to_vbranch_commit(
                project_repository,
                &default_target,
                &commit,
                Some(&upstream_commits),
            )?;
            commits.push(commit);
        }

        // if the branch is not applied, check to see if it's mergeable and up to date
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
            notes: branch.notes.to_string(),
            active: branch.applied,
            files: vfiles,
            order: branch.order,
            commits,
            mergeable,
            merge_conflicts,
            upstream: branch.upstream.clone(),
            conflicted: conflicts::is_resolving(project_repository),
            base_current,
        };
        branches.push(branch);
    }
    branches.sort_by(|a, b| a.order.cmp(&b.order));
    Ok(branches)
}

fn list_commit_files(
    project_repository: &project_repository::Repository,
    commit: &git::Commit,
) -> Result<Vec<VirtualBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff = diff::trees(
        &project_repository.git_repository,
        &parent_tree,
        &commit_tree,
    )?;
    let hunks_by_filepath = hunks_by_filepath(project_repository, &diff);
    Ok(hunks_to_files(
        project_repository,
        &hunks_by_filepath
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>(),
    ))
}

pub fn commit_to_vbranch_commit(
    repository: &project_repository::Repository,
    target: &target::Target,
    commit: &git::Commit,
    upstream_commits: Option<&HashMap<git::Oid, bool>>,
) -> Result<VirtualBranchCommit> {
    let timestamp = commit.time().seconds() as u128;
    let signature = commit.author();
    let message = commit.message().unwrap().to_string();
    let sha = commit.id().to_string();

    let is_remote = match upstream_commits {
        Some(commits) => commits.contains_key(&commit.id()),
        None => true,
    };

    let files = if is_remote {
        vec![]
    } else {
        list_commit_files(repository, commit).context("failed to list commit files")?
    };

    let is_integrated = is_commit_integrated(repository, target, commit)?;

    let commit = VirtualBranchCommit {
        id: sha,
        created_at: timestamp * 1000,
        author: Author::from(signature),
        description: message,
        is_remote,
        files,
        is_integrated,
    };

    Ok(commit)
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

    let mut all_virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<branch::Branch>>();
    all_virtual_branches.sort_by_key(|branch| branch.order);

    let order = create
        .order
        .unwrap_or(all_virtual_branches.len())
        .clamp(0, all_virtual_branches.len());

    let branch_writer = branch::Writer::new(gb_repository);

    // make space for the new branch
    for (i, branch) in all_virtual_branches.iter().enumerate() {
        let mut branch = branch.clone();
        let new_order = if i < order { i } else { i + 1 };
        if branch.order != new_order {
            branch.order = new_order;
            branch_writer
                .write(&branch)
                .context("failed to write branch")?;
        }
    }

    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get elapsed time")?
        .as_millis();

    let name: String = create
        .name
        .as_ref()
        .map(|name| name.to_string())
        .unwrap_or_else(|| {
            dedup(
                &all_virtual_branches
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>(),
                "Virtual branch",
            )
        });

    let mut branch = Branch {
        id: Uuid::new_v4().to_string(),
        name,
        notes: "".to_string(),
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
    project_repository: &project_repository::Repository,
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
        let unique_branch_name = Iterator::new(&current_session_reader)
            .context("failed to create branch iterator")?
            .collect::<Result<Vec<branch::Branch>, reader::Error>>()
            .context("failed to read virtual branches")?
            .into_iter()
            .filter(|branch| branch.name == name)
            .collect::<Vec<_>>()
            .is_empty();
        if unique_branch_name {
            let old_branch_name = name_to_branch(&branch.name.clone());
            let old_refname = format!("refs/gitbutler/{}", old_branch_name);

            let branch_name = name_to_branch(&name.clone());
            let refname = format!("refs/gitbutler/{}", branch_name);

            branch.name = name;

            // rename the ref
            let repo = &project_repository.git_repository;
            if let Ok(mut reference) = repo
                .find_reference(&old_refname)
                .context("failed to find reference")
            {
                reference
                    .rename(&refname, true, "gb")
                    .context("failed to rename reference")?;
            }
        } else {
            bail!("branch name {} already exists", name);
        }
    };

    if let Some(notes) = branch_update.notes {
        branch.notes = notes;
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
    project_repository: &project_repository::Repository,
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

    // remove refs/butler reference
    let repo = &project_repository.git_repository;
    let branch_name = name_to_branch(&branch.name);
    let ref_name = format!("refs/gitbutler/{}", branch_name);
    println!("deleting ref: {}", ref_name);
    if let Ok(mut reference) = repo.find_reference(&ref_name) {
        println!("FOUND {}", ref_name);
        reference
            .delete()
            .context(format!("failed to delete {}", ref_name))?;
    }

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
        .skip(1) // skip the first line which is the diff header
        .filter(|line| line.starts_with('+') || line.starts_with('-')) // exclude context lines
        .collect::<Vec<_>>()
        .join("\n");
    format!("{:x}", md5::compute(addition))
}

pub fn hunks_by_filepath(
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
                    binary: hunk.binary,
                    hash: diff_hash(&hunk.diff),
                    locked: false,
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

    let default_target = match get_default_target(&current_session_reader)
        .context("failed to read default target")?
    {
        Some(target) => target,
        None => {
            return Ok(vec![]);
        }
    };

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    let applied_virtual_branches = virtual_branches
        .iter()
        .filter(|branch| branch.applied)
        .cloned()
        .collect::<Vec<_>>();

    let applied_status = get_applied_status(
        gb_repository,
        project_repository,
        &default_target,
        applied_virtual_branches,
    )?;

    let non_applied_virtual_branches = virtual_branches
        .into_iter()
        .filter(|branch| !branch.applied)
        .collect::<Vec<_>>();

    let non_applied_status = get_non_applied_status(
        project_repository,
        &default_target,
        non_applied_virtual_branches,
    )?;

    Ok(applied_status
        .into_iter()
        .chain(non_applied_status.into_iter())
        .collect())
}

// given a list of non applied virtual branches, return the status of each file, comparing the default target with
// virtual branch latest tree
//
// ownerships are not taken into account here, as they are not relevant for non applied branches
fn get_non_applied_status(
    project_repository: &project_repository::Repository<'_>,
    default_target: &target::Target,
    virtual_branches: Vec<branch::Branch>,
) -> Result<Vec<(branch::Branch, Vec<VirtualBranchFile>)>> {
    let hunks_by_branch = virtual_branches
        .into_iter()
        .map(
            |branch| -> Result<(branch::Branch, Vec<VirtualBranchHunk>)> {
                if branch.applied {
                    bail!("branch {} is applied", branch.name);
                }
                let branch_tree = project_repository
                    .git_repository
                    .find_tree(branch.tree)
                    .context(format!("failed to find tree {}", branch.tree))?;
                let target_tree = project_repository
                    .git_repository
                    .find_commit(default_target.sha)
                    .context("failed to find target commit")?
                    .tree()
                    .context("failed to find target tree")?;

                let diff = diff::trees(
                    &project_repository.git_repository,
                    &target_tree,
                    &branch_tree,
                )?;
                let timestamp_by_hash = branch
                    .ownership
                    .files
                    .iter()
                    .flat_map(|file| {
                        file.hunks
                            .iter()
                            .filter_map(|hunk| match (hunk.hash.as_ref(), hunk.timestam_ms()) {
                                (Some(hash), Some(timestamp_ms)) => Some((hash, timestamp_ms)),
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>()
                    })
                    .collect::<HashMap<_, _>>();
                let hunks_by_filepath = hunks_by_filepath(project_repository, &diff);
                let hunks_by_filepath = hunks_by_filepath
                    .values()
                    .flatten()
                    .map(|hunk| {
                        let modified_at = timestamp_by_hash
                            .get(&hunk.hash)
                            .cloned()
                            .unwrap_or(hunk.modified_at);
                        VirtualBranchHunk {
                            modified_at,
                            ..hunk.clone()
                        }
                    })
                    .collect::<Vec<_>>();
                Ok((branch, hunks_by_filepath))
            },
        )
        .collect::<Result<Vec<_>>>()?;

    Ok(group_virtual_hunks(project_repository, &hunks_by_branch))
}

// given a list of applied virtual branches, return the status of each file, comparing the default target with
// the working directory
//
// ownerships are updated if nessessary
fn get_applied_status(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository<'_>,
    default_target: &target::Target,
    mut virtual_branches: Vec<branch::Branch>,
) -> Result<Vec<(branch::Branch, Vec<VirtualBranchFile>)>> {
    let diff = diff::workdir(
        &project_repository.git_repository,
        &default_target.sha,
        &diff::Options::default(),
    )
    .context("failed to diff")?;

    let mut hunks_by_filepath = hunks_by_filepath(project_repository, &diff);

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
        if !branch.applied {
            bail!("branch {} is not applied", branch.name);
        }

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
            &format!(
                "{}:{}-{}-{}-{}",
                hunk.file_path.display(),
                hunk.start,
                hunk.end,
                hunk.hash,
                hunk.modified_at,
            )
            .parse()
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

    let hunks_by_branch = hunks_by_branch_id
        .into_iter()
        .map(|(branch_id, hunks)| {
            (
                virtual_branches
                    .iter()
                    .find(|b| b.id.eq(&branch_id))
                    .unwrap()
                    .clone(),
                hunks,
            )
        })
        .collect::<Vec<_>>();

    let mut files_by_branch = group_virtual_hunks(project_repository, &hunks_by_branch);
    files_by_branch.sort_by(|a, b| a.0.order.cmp(&b.0.order));

    Ok(files_by_branch)
}

fn hunks_to_files(
    project_repository: &project_repository::Repository,
    hunks: &[VirtualBranchHunk],
) -> Vec<VirtualBranchFile> {
    hunks
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
            binary: hunks.iter().any(|h| h.binary),
            modified_at: hunks.iter().map(|h| h.modified_at).max().unwrap_or(0),
            conflicted: conflicts::is_conflicting(
                project_repository,
                Some(&file_path.display().to_string()),
            )
            .unwrap_or(false),
        })
        .collect::<Vec<_>>()
}

fn group_virtual_hunks(
    project_repository: &project_repository::Repository,
    hunks_by_branch: &[(branch::Branch, Vec<VirtualBranchHunk>)],
) -> Vec<(branch::Branch, Vec<VirtualBranchFile>)> {
    hunks_by_branch
        .iter()
        .map(|(branch, hunks)| {
            let mut files = hunks_to_files(project_repository, hunks);
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

            (branch.clone(), files)
        })
        .collect::<Vec<_>>()
}

// this function takes a list of file ownership,
// constructs a tree from those changes on top of the target
// and writes it as a new tree for storage
pub fn write_tree(
    project_repository: &project_repository::Repository,
    target: &target::Target,
    files: &Vec<VirtualBranchFile>,
) -> Result<git::Oid> {
    // read the base sha into an index
    let git_repository = &project_repository.git_repository;

    let head_commit = git_repository.find_commit(target.sha)?;
    let base_tree = head_commit.tree()?;

    let mut builder = git_repository.treebuilder(Some(&base_tree));
    // now update the index with content in the working directory for each file
    for file in files {
        // convert this string to a Path
        let rel_path = std::path::Path::new(&file.path);
        let full_path = project_repository.path().join(rel_path);

        // if file exists
        if full_path.exists() {
            // if file is executable, use 755, otherwise 644
            let mut filemode = git::FileMode::Blob;
            // check if full_path file is executable
            if let Ok(metadata) = std::fs::symlink_metadata(&full_path) {
                if metadata.permissions().mode() & 0o111 != 0 {
                    filemode = git::FileMode::BlobExecutable;
                }
                if metadata.file_type().is_symlink() {
                    filemode = git::FileMode::Link;
                }
            }

            // get the blob
            if filemode == git::FileMode::Link {
                // it's a symlink, make the content the path of the link
                let link_target = std::fs::read_link(&full_path)?;
                // make link_target into a relative path
                let link_target = link_target.strip_prefix(project_repository.path()).unwrap();
                // create a blob where the content is that target path string
                // make a [u8] out of the PathBuf
                let path_str = link_target.to_str().unwrap();
                let bytes: &[u8] = path_str.as_bytes();

                let blob_oid = git_repository.blob(bytes)?;
                builder.upsert(rel_path, blob_oid, filemode);
            } else if let Ok(tree_entry) = base_tree.get_path(rel_path) {
                if file.binary {
                    let new_blob_oid = &file.hunks[0].diff;
                    // convert string to Oid
                    let new_blob_oid = new_blob_oid.parse().context("failed to diff as oid")?;
                    builder.upsert(rel_path, new_blob_oid, filemode);
                } else {
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
                    builder.upsert(rel_path, new_blob_oid, filemode);
                }
            } else {
                // create a git blob from a file on disk
                let blob_oid = git_repository.blob_path(&full_path)?;
                builder.upsert(rel_path, blob_oid, filemode);
            }
        } else if base_tree.get_path(rel_path).is_ok() {
            // remove file from index if it exists in the base tree
            builder.remove(rel_path);
        } else {
            // file not in index or base tree, do nothing
            // this is the
        }
    }

    // now write out the tree
    let tree_oid = builder.write().context("failed to write updated tree")?;

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
        if let Ok(content) = std::str::from_utf8(blob.content()) {
            println!("    blob: {:?}", content);
        } else {
            println!("    blob: BINARY");
        }
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

    let default_target = gb_repository
        .default_target()
        .context("failed to get default target")?
        .context("no default target set")?;

    // get the files to commit
    let statuses = get_status_by_branch(gb_repository, project_repository)
        .context("failed to get status by branch")?;

    match statuses.iter().find(|(branch, _)| branch.id == branch_id) {
        None => bail!("branch {} not found", branch_id),
        Some((branch, files)) => {
            let tree_oid = write_tree(project_repository, &default_target, files)?;

            let git_repository = &project_repository.git_repository;
            let parent_commit = git_repository
                .find_commit(branch.head)
                .context(format!("failed to find commit {:?}", branch.head))?;
            let tree = git_repository
                .find_tree(tree_oid)
                .context(format!("failed to find tree {:?}", tree_oid))?;

            // now write a commit, using a merge parent if it exists
            let (author, committer) = gb_repository
                .git_signatures()
                .context("failed to get git signatures")?;
            let extra_merge_parent = conflicts::merge_parent(project_repository)
                .context("failed to get merge parent")?;

            let commit_oid = match extra_merge_parent {
                Some(merge_parent) => {
                    let merge_parent = git_repository
                        .find_commit(merge_parent)
                        .context(format!("failed to find merge parent {:?}", merge_parent))?;
                    let commit_oid = git_repository
                        .commit(
                            None,
                            &author,
                            &committer,
                            message,
                            &tree,
                            &[&parent_commit, &merge_parent],
                        )
                        .context("failed to commit")?;
                    conflicts::clear(project_repository).context("failed to clear conflicts")?;
                    commit_oid
                }
                None => git_repository.commit(
                    None,
                    &author,
                    &committer,
                    message,
                    &tree,
                    &[&parent_commit],
                )?,
            };

            // update the virtual branch head
            let writer = branch::Writer::new(gb_repository);
            writer
                .write(&Branch {
                    tree: tree_oid,
                    head: commit_oid,
                    ..branch.clone()
                })
                .context("failed to write branch")?;

            super::integration::update_gitbutler_integration(gb_repository, project_repository)
                .context("failed to update gitbutler integration")?;

            Ok(())
        }
    }
}

pub fn name_to_branch(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error(transparent)]
    Repository(#[from] project_repository::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn push(
    project_repository: &project_repository::Repository,
    gb_repository: &gb_repository::Repository,
    branch_id: &str,
    key: &Key,
) -> Result<(), PushError> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")
        .map_err(PushError::Other)?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")
        .map_err(PushError::Other)?;

    let branch_reader = branch::Reader::new(&current_session_reader);
    let branch_writer = branch::Writer::new(gb_repository);

    let mut vbranch = branch_reader
        .read(branch_id)
        .context("failed to read branch")
        .map_err(PushError::Other)?;

    let remote_branch = if let Some(upstream_branch) = vbranch.upstream.as_ref() {
        upstream_branch.clone()
    } else {
        let remote_branch = match get_default_target(&current_session_reader)? {
            Some(target) => format!(
                "refs/remotes/{}/{}",
                target.branch.remote(),
                name_to_branch(&vbranch.name)
            )
            .parse::<git::RemoteBranchName>()
            .unwrap(),
            None => return Err(PushError::Other(anyhow::anyhow!("no default target set"))),
        };
        let remote_branches = project_repository.git_remote_branches()?;
        let existing_branches = remote_branches
            .iter()
            .map(|name| name.branch())
            .collect::<Vec<_>>();
        remote_branch.with_branch(&name_to_branch(&dedup_fmt(
            &existing_branches,
            remote_branch.branch(),
            "-",
        )))
    };

    project_repository.push(&vbranch.head, &remote_branch, key)?;

    vbranch.upstream = Some(remote_branch.clone());
    branch_writer
        .write(&vbranch)
        .context("failed to write target branch after push")?;

    project_repository.fetch(remote_branch.remote(), key)?;

    Ok(())
}

pub fn mark_all_unapplied(gb_repository: &gb_repository::Repository) -> Result<()> {
    let current_session = gb_repository.get_or_create_current_session()?;
    let session_reader = sessions::Reader::open(gb_repository, &current_session)?;
    let branch_iterator = super::Iterator::new(&session_reader)?;
    let branch_writer = super::branch::Writer::new(gb_repository);
    branch_iterator
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .map(|branch| {
            branch_writer.write(&super::Branch {
                applied: false,
                ..branch
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .context("failed to write branches")?;
    Ok(())
}

fn is_commit_integrated(
    project_repository: &project_repository::Repository,
    target: &target::Target,
    commit: &git::Commit,
) -> Result<bool> {
    let remote_branch = project_repository
        .git_repository
        .find_branch(&target.branch.clone().into())?;
    let remote_head = remote_branch.peel_to_commit()?;
    let upstream_commits = project_repository.l(
        remote_head.id(),
        project_repository::LogUntil::Commit(target.sha),
    )?;

    if target.sha.eq(&commit.id()) {
        // could not be integrated if heads are the same.
        return Ok(false);
    }

    if upstream_commits.is_empty() {
        // could not be integrated if there is nothing new upstream.
        return Ok(false);
    }

    let merge_base = project_repository
        .git_repository
        .merge_base(target.sha, commit.id())?;
    if merge_base.eq(&commit.id()) {
        // if merge branch is the same as branch head and there are upstream commits
        // then it's integrated
        return Ok(true);
    }

    let merge_commit = project_repository.git_repository.find_commit(merge_base)?;
    let merge_tree = merge_commit.tree()?;
    let upstream = project_repository
        .git_repository
        .find_commit(remote_head.id())?;
    let upstream_tree = upstream.tree()?;
    let upstream_tree_oid = upstream_tree.id();

    // try to merge our tree into the upstream tree
    let mut merge_index = project_repository
        .git_repository
        .merge_trees(&merge_tree, &upstream_tree, &commit.tree()?)
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        return Ok(false);
    }

    let merge_tree_oid = merge_index
        .write_tree_to(&project_repository.git_repository)
        .context("failed to write tree")?;

    // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
    // then the vbranch is fully merged
    Ok(merge_tree_oid == upstream_tree_oid)
}
