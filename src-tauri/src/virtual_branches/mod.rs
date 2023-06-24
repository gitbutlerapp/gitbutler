pub mod branch;
mod iterator;
pub mod target;

use std::{
    collections::{HashMap, HashSet},
    path, time, vec,
};

use anyhow::{bail, Context, Result};
use filetime::FileTime;
use serde::Serialize;

pub use branch::Branch;
pub use iterator::BranchIterator as Iterator;
use uuid::Uuid;

use crate::{gb_repository, project_repository, reader, sessions};

use self::branch::Ownership;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranch {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
    pub commits: Vec<VirtualBranchCommit>,
}

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

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchFile {
    pub id: String,
    pub path: String,
    pub hunks: Vec<VirtualBranchHunk>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchHunk {
    pub id: String,
    pub name: String,
    pub diff: String,
    pub modified_at: u128,
    pub file_path: String,
}

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

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => Ok(target),
        Err(reader::Error::NotFound) => return Ok(vec![]),
        Err(e) => Err(e),
    }
    .context("failed to read default target")?;

    let main_oid = default_target.sha;

    let current_time = time::SystemTime::now();
    let too_old = time::Duration::from_secs(86_400 * 90); // 90 days (3 months) is too old

    let repo = &project_repository.git_repository;
    let mut branches: Vec<RemoteBranch> = Vec::new();
    for branch in repo.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = branch?;
        let branch_name = branch.get().name().unwrap();
        let upstream_branch = branch.upstream();
        match branch.get().target() {
            Some(branch_oid) => {
                // get the branch ref
                let branch_commit = repo.find_commit(branch_oid).ok().unwrap();

                // figure out if the last commit on this branch is too old to consider
                let branch_time = branch_commit.time();
                // convert git::Time to SystemTime
                let branch_time = time::UNIX_EPOCH
                    + time::Duration::from_secs(branch_time.seconds().try_into().unwrap());
                let duration = current_time.duration_since(branch_time).unwrap();
                if duration > too_old {
                    continue;
                }

                let mut revwalk = repo.revwalk().unwrap();
                revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();
                revwalk.push(main_oid).unwrap();
                revwalk.hide(branch_oid).unwrap();

                let mut count_behind = 0;
                for oid in revwalk {
                    if oid.unwrap() == branch_oid {
                        break;
                    }
                    count_behind += 1;
                    if count_behind > 100 {
                        break;
                    }
                }

                let mut revwalk2 = repo.revwalk().unwrap();
                revwalk2.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();
                revwalk2.push(branch_oid).unwrap();
                revwalk2.hide(main_oid).unwrap();

                let mut min_time = None;
                let mut max_time = None;
                let mut count_ahead = 0;
                let mut authors = HashSet::new();
                for oid in revwalk2 {
                    let oid = oid.unwrap();
                    if oid == main_oid {
                        break;
                    }
                    let commit = repo.find_commit(oid).ok().unwrap();
                    let timestamp = commit.time().seconds() as u128;

                    if min_time.is_none() || timestamp < min_time.unwrap() {
                        min_time = Some(timestamp);
                    }

                    if max_time.is_none() || timestamp > max_time.unwrap() {
                        max_time = Some(timestamp);
                    }

                    // find the signature for this commit
                    let commit = repo.find_commit(oid).ok().unwrap();
                    let signature = commit.author();
                    authors.insert(signature.email().unwrap().to_string());

                    count_ahead += 1;
                }

                let upstream_branch_name = match upstream_branch {
                    Ok(upstream_branch) => upstream_branch.get().name().unwrap_or("").to_string(),
                    Err(_) => "".to_string(),
                };

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
                });
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
                });
            }
        }
    }
    Ok(branches)
}

// just for debugging for now
fn print_diff(diff: git2::Diff) -> Result<()> {
    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        println!(
            "delta: {:?} {:?}",
            line.origin(),
            std::str::from_utf8(line.content()).unwrap()
        );
        true
    })?;
    Ok(())
}

pub fn list_virtual_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Vec<VirtualBranch>> {
    let mut branches: Vec<VirtualBranch> = Vec::new();
    let current_session = gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");

    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .expect("failed to open current session reader");

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => Ok(target),
        Err(reader::Error::NotFound) => return Ok(vec![]),
        Err(e) => Err(e),
    }
    .context("failed to read default target")?;
    let default_sha = default_target.sha.clone();

    let statuses = get_status_by_branch(gb_repository, project_repository)?;
    for (branch, files) in &statuses {
        let mut vfiles = vec![];

        // check if head tree does not match target tree
        // if so, we diff the head tree and the new write_tree output to see what is new and filter the hunks to just those
        if default_sha != branch.head {
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
                })
                .collect::<Vec<_>>();
        } else {
            for file in files {
                vfiles.push(file.clone());
            }
        }

        let mut commits = vec![];

        // find all commits on head that are not on target.sha
        let repo = &project_repository.git_repository;
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
            let commit = VirtualBranchCommit {
                id: sha,
                created_at: timestamp * 1000,
                author_name: name,
                author_email: email,
                description: message,
                is_remote: false,
            };
            commits.push(commit);
        }

        let branch = VirtualBranch {
            id: branch.id.to_string(),
            name: branch.name.to_string(),
            active: branch.applied,
            files: vfiles,
            commits,
        };
        branches.push(branch);
    }
    Ok(branches)
}

pub fn create_virtual_branch(
    gb_repository: &gb_repository::Repository,
    name: &str,
) -> Result<String> {
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

    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get elapsed time")?
        .as_millis();

    let branch = Branch {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        applied: true,
        upstream: "".to_string(),
        tree: tree.id(),
        head: default_target.sha,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: vec![],
    };

    let writer = branch::Writer::new(gb_repository);
    writer.write(&branch).context("failed to write branch")?;
    Ok(branch.id)
}

pub fn move_files(
    gb_repository: &gb_repository::Repository,
    dst_branch_id: &str,
    to_move: &Vec<branch::Ownership>,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    let writer = branch::Writer::new(gb_repository);

    let mut target_branch = virtual_branches
        .iter()
        .find(|b| b.id == dst_branch_id)
        .context("failed to find target branch")?
        .clone();

    let update_list = |list: Vec<Branch>, update: &Branch| {
        list.iter()
            .map(|branch| {
                if branch.id.eq(&update.id) {
                    update.clone()
                } else {
                    branch.clone()
                }
            })
            .collect::<Vec<_>>()
    };

    for ownership in to_move {
        let source_branches = if ownership.ranges.is_empty() {
            // find all branches that own any part of the file
            virtual_branches
                .clone()
                .into_iter()
                .filter(|b| {
                    b.ownership
                        .iter()
                        .any(|o| o.file_path.eq(&ownership.file_path))
                })
                .collect::<Vec<_>>()
        } else {
            let owner = find_owner(&virtual_branches, ownership)
                .context(format!("failed to find owner branch for {}", ownership))?
                .clone();
            vec![owner]
        };

        for mut source_branch in source_branches {
            source_branch.take(ownership);
            source_branch
                .ownership
                .sort_by(|a, b| a.file_path.cmp(&b.file_path));
            source_branch.ownership.dedup();

            writer
                .write(&source_branch)
                .context(format!("failed to write source branch for {}", ownership))?;
            virtual_branches = update_list(virtual_branches, &source_branch);
        }

        target_branch.put(ownership);
        target_branch
            .ownership
            .sort_by(|a, b| a.file_path.cmp(&b.file_path));
        target_branch.ownership.dedup();

        writer
            .write(&target_branch)
            .context(format!("failed to write target branch for {}", ownership))?;
        virtual_branches = update_list(virtual_branches, &target_branch);

        log::info!(
            "{}: moved {} to branch {}",
            gb_repository.project_id,
            ownership,
            target_branch.name
        );
    }

    Ok(())
}

fn find_owner(stack: &[branch::Branch], needle: &branch::Ownership) -> Option<branch::Branch> {
    let explicitly_owned_by = stack.iter().find(|b| {
        b.ownership
            .iter()
            .filter(|o| !o.ranges.is_empty())
            .any(|o| o.contains(needle))
    });
    let implicitly_owned_by = stack.iter().find(|b| b.contains(needle));
    explicitly_owned_by.or(implicitly_owned_by).cloned()
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
    let mut mtimes = HashMap::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        let file_path = delta.new_file().path().unwrap_or_else(|| {
            delta
                .old_file()
                .path()
                .expect("failed to get file name from diff")
        });

        let hunk_id = if let Some(hunk) = hunk {
            format!(
                "{}:{}-{}",
                file_path.display(),
                hunk.new_start(),
                hunk.new_start() + hunk.new_lines()
            )
        } else {
            // no hunk, so we're in the header, skip it
            return true;
        };

        let mtime = match mtimes.get(file_path) {
            Some(mtime) => *mtime,
            None => {
                let file_path = project_repository
                    .git_repository
                    .workdir()
                    .unwrap()
                    .join(file_path);
                let mtime = 0;
                if let Ok(metadata) = file_path.metadata() {
                    let mtime = FileTime::from_last_modification_time(&metadata);
                    // convert seconds and nanoseconds to milliseconds
                    let mtime = mtime.seconds() as u128 * 1000;
                    mtimes.insert(file_path, mtime);
                }
                mtime
            }
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

        true
    })
    .context("failed to print diff")?;

    if let Some(file_path) = current_file_path {
        let mtime = match mtimes.get(&file_path) {
            Some(mtime) => *mtime,
            None => {
                let file_path = project_repository
                    .git_repository
                    .workdir()
                    .unwrap()
                    .join(&file_path);

                let metadata = file_path.metadata().unwrap();
                let mtime = FileTime::from_last_modification_time(&metadata);
                // convert seconds and nanoseconds to milliseconds
                let mtime = mtime.seconds() as u128 * 1000;
                mtimes.insert(file_path, mtime);
                mtime
            }
        };

        let file_path = file_path.to_str().unwrap().to_string();
        hunks_by_filepath
            .entry(file_path.clone())
            .or_default()
            .push(VirtualBranchHunk {
                id: current_hunk_id.as_ref().unwrap().to_string(),
                name: "".to_string(),
                diff: current_diff,
                modified_at: mtime,
                file_path,
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

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => Ok(target),
        Err(reader::Error::NotFound) => {
            println!("  no base sha set, run butler setup");
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

    let hunks_by_filepath = diff_to_hunks_by_filepath(diff, project_repository)?;

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to read virtual branches")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;

    if virtual_branches.is_empty() {
        println!("  no virtual branches, run butler setup");
        return Ok(vec![]);
    }

    // sort by created timestamp so that default selected branch is the earliest created one
    virtual_branches.sort_by(|a, b| a.created_timestamp_ms.cmp(&b.created_timestamp_ms));

    // select default branch
    let first_applied_id = virtual_branches
        .iter()
        .find(|b| b.applied)
        .map(|b| b.id.clone());
    let branch_reader = branch::Reader::new(&current_session_reader);
    let default_branch_id = branch_reader
        .read_selected()
        .context("failed to read selected branch")?
        .or(first_applied_id);

    // now, distribute hunks to the branches
    let mut hunks_by_branch_id: HashMap<String, Vec<VirtualBranchHunk>> = virtual_branches
        .iter()
        .map(|b| (b.id.clone(), Vec::new()))
        .collect();
    let all_hunks = hunks_by_filepath.values().flatten().collect::<Vec<_>>();
    for hunk in all_hunks {
        let hunk_ownership = Ownership::try_from(&hunk.id)?;

        let owned_by = find_owner(&virtual_branches, &hunk_ownership);
        if let Some(branch) = owned_by {
            if hunk.file_path == "src-tauri/Cargo.lock" {
                println!("explicit branch: {:?}", branch);
            }

            hunks_by_branch_id
                .entry(branch.id.clone())
                .or_default()
                .push(hunk.clone());
            continue;
        }

        if default_branch_id.is_none() {
            bail!("no branch found for {}", hunk_ownership);
        }

        // put ownership into the virtual branch
        let mut default_branch = virtual_branches
            .iter()
            .find(|b| b.id.eq(default_branch_id.as_ref().unwrap()))
            .unwrap()
            .clone();

        default_branch.put(&hunk_ownership);

        // write the updated data back
        let writer = branch::Writer::new(gb_repository);
        writer
            .write(&default_branch)
            .context("failed to write branch")?;

        // update virtual branches list for future usage
        virtual_branches = virtual_branches
            .iter()
            .map(|branch| {
                if branch.id.eq(&default_branch.id) {
                    default_branch.clone()
                } else {
                    branch.clone()
                }
            })
            .collect::<Vec<_>>();

        hunks_by_branch_id
            .entry(default_branch.id.clone())
            .or_default()
            .push(hunk.clone());
    }

    let mut statuses: Vec<(branch::Branch, Vec<VirtualBranchFile>)> = vec![];
    for (branch_id, hunks) in hunks_by_branch_id {
        let branch = virtual_branches
            .iter()
            .find(|b| b.id.eq(&branch_id))
            .unwrap()
            .clone();

        let files = hunks
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
                hunks,
            })
            .collect::<Vec<_>>();

        statuses.push((branch, files));
    }
    statuses.sort_by(|a, b| a.0.name.cmp(&b.0.name));

    Ok(statuses)
}

fn get_default_target(gb_repository: &gb_repository::Repository) -> Result<target::Target> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .expect("failed to open current session reader");

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => target,
        Err(e) => panic!("failed to read default target: {}", e),
    };
    Ok(default_target)
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
    println!("updating branch target");
    Ok(())
}

fn write_tree(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    files: &Vec<VirtualBranchFile>,
) -> Result<git2::Oid> {
    let default_target = get_default_target(gb_repository)?;

    // read the base sha into an index
    let git_repository = &project_repository.git_repository;
    let base_commit = git_repository.find_commit(default_target.sha).unwrap();
    let base_tree = base_commit.tree().unwrap();
    let mut index = git_repository.index().unwrap();
    index.read_tree(&base_tree).unwrap();

    // now update the index with content in the working directory for each file
    for file in files {
        // convert this string to a Path
        let file = std::path::Path::new(&file.path);

        // TODO: deal with removals too
        index.add_path(file).unwrap();
    }

    // now write out the tree
    let tree_oid = index.write_tree().unwrap();
    Ok(tree_oid)
}

pub fn commit(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
    branch_id: &str,
    message: &str,
) -> Result<()> {
    // get the files to commit
    let statuses = get_status_by_branch(gb_repository, project_repository)
        .expect("failed to get status by branch");
    for (mut branch, files) in statuses {
        if branch.id == branch_id {
            let tree_oid = write_tree(gb_repository, project_repository, &files)?;
            if tree_oid != branch.tree {
                let git_repository = &project_repository.git_repository;
                let parent_commit = git_repository.find_commit(branch.head).unwrap();
                let tree = git_repository.find_tree(tree_oid).unwrap();
                // now write a commit
                let (author, committer) = gb_repository.git_signatures().unwrap();
                let commit_oid = git_repository
                    .commit(None, &author, &committer, message, &tree, &[&parent_commit])
                    .unwrap();

                // update the virtual branch head
                branch.tree = tree_oid;
                branch.head = commit_oid;
                let writer = branch::Writer::new(gb_repository);
                writer.write(&branch).unwrap();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
    fn commit_on_branch_then_change_file_then_get_status() -> Result<()> {
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
        })?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write_selected(&Some(branch1_id.clone()))?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\n",
        )?;

        let branches = list_virtual_branches(&gb_repo, &project_repository)?;
        let branch = &branches[0];
        assert_eq!(branch.files.len(), 1);
        assert_eq!(branch.commits.len(), 0);

        // commit
        commit(&gb_repo, &project_repository, &branch1_id, "test commit")?;

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
    fn create_branch() -> Result<()> {
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
        })?;

        create_virtual_branch(&gb_repo, "test_branch").expect("failed to create virtual branch");

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

        let branches = iterator::BranchIterator::new(&current_session_reader)?
            .collect::<Result<Vec<branch::Branch>, reader::Error>>()
            .expect("failed to read branches");
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "test_branch");

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
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");
        branch::Writer::new(&gb_repo).write_selected(&Some(branch1_id.clone()))?;

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
    fn test_move_hunks_entire_file_multiple_sources() -> Result<()> {
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
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n",
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
        })?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )?;

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write_selected(&Some(branch2_id.clone()))?;

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch2 = branch_reader.read(&branch2_id)?;
        branch_writer.write(&branch::Branch {
            ownership: vec!["test.txt:1-5".try_into()?],
            ..branch2
        })?;
        let branch1 = branch_reader.read(&branch1_id)?;
        branch_writer.write(&branch::Branch {
            ownership: vec!["test.txt:8-12".try_into()?],
            ..branch1
        })?;

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
        assert_eq!(files_by_branch_id[&branch2_id][0].hunks.len(), 1);

        move_files(&gb_repo, &branch2_id, &vec!["test.txt".try_into()?])
            .expect("failed to move hunks");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        println!("{:#?}", statuses);

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id][0].hunks.len(), 2);

        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(branch_reader.read(&branch1_id)?.ownership, vec![]);
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership,
            vec!["test.txt:1-5,8-12".try_into()?]
        );

        Ok(())
    }

    #[test]
    fn test_move_hunks_entire_file_single_source_full_file() -> Result<()> {
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
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        let branch_writer = branch::Writer::new(&gb_repo);
        branch::Writer::new(&gb_repo).write_selected(&Some(branch1_id.clone()))?;

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch_reader = branch::Reader::new(&current_session_reader);
        let branch1 = branch_reader.read(&branch1_id)?;
        branch_writer.write(&branch::Branch {
            ownership: vec!["test.txt".try_into()?],
            ..branch1
        })?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        move_files(&gb_repo, &branch2_id, &vec!["test.txt".try_into()?])
            .expect("failed to move hunks");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(branch_reader.read(&branch1_id)?.ownership, vec![]);
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership,
            vec!["test.txt".try_into()?]
        );

        Ok(())
    }

    #[test]
    fn test_move_hunks_entire_file_single_source() -> Result<()> {
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
        })?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        branch::Writer::new(&gb_repo).write_selected(&Some(branch1_id.clone()))?;

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 1);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 0);

        move_files(&gb_repo, &branch2_id, &vec!["test.txt".try_into()?])
            .expect("failed to move hunks");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);

        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;

        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(branch_reader.read(&branch1_id)?.ownership, vec![]);
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership,
            vec!["test.txt".try_into()?]
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
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n",
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
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        branch::Writer::new(&gb_repo).write_selected(&Some(branch1_id.clone()))?;

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

        move_files(&gb_repo, &branch2_id, &vec!["test.txt:1-5".try_into()?])
            .expect("failed to move hunks");

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
            branch_reader.read(&branch1_id)?.ownership,
            vec!["test.txt:8-12".try_into()?]
        );
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership,
            vec!["test.txt:1-5".try_into()?]
        );

        Ok(())
    }

    #[test]
    fn test_move_hunks_partial_implicity() -> Result<()> {
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
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n",
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
        })?;

        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        let branch_writer = branch::Writer::new(&gb_repo);
        branch_writer.write_selected(&Some(branch1_id.clone()))?;

        // update ownership to be implicit
        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let branch1 = branch::Reader::new(&current_session_reader).read(&branch1_id)?;
        branch_writer.write(&Branch {
            ownership: vec!["test.txt".try_into()?],
            ..branch1
        })?;

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

        move_files(&gb_repo, &branch2_id, &vec!["test.txt:1-5".try_into()?])
            .expect("failed to move hunks");

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

        let branch_reader = branch::Reader::new(&current_session_reader);
        assert_eq!(
            branch_reader.read(&branch1_id)?.ownership,
            vec!["test.txt".try_into()?]
        );
        assert_eq!(
            branch_reader.read(&branch2_id)?.ownership,
            vec!["test.txt:1-5".try_into()?]
        );

        Ok(())
    }
}
