pub mod branch;
mod iterator;
pub mod target;

use std::{collections::HashMap, time, vec};

use anyhow::{Context, Result};
use serde::Serialize;

pub use branch::Branch;
pub use iterator::BranchIterator as Iterator;
use uuid::Uuid;

use crate::{gb_repository, project_repository, reader, sessions};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranch {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
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

pub fn list_virtual_branches(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Vec<VirtualBranch>> {
    let mut branches: Vec<VirtualBranch> = Vec::new();

    let statuses = get_status_by_branch(gb_repository, project_repository)?;
    for (branch, files) in &statuses {
        let mut vfiles = vec![];
        for file in files {
            vfiles.push(file.clone());
        }
        let branch = VirtualBranch {
            id: branch.id.to_string(),
            name: branch.name.to_string(),
            active: branch.applied,
            files: vfiles,
        };
        branches.push(branch);
    }
    Ok(branches)
}

pub fn create_virtual_branch(
    gb_repository: &gb_repository::Repository,
    name: String,
) -> Result<String> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let target_reader = target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => Ok(target),
        Err(reader::Error::NotFound) => return Ok("".to_string()),
        Err(e) => Err(e),
    }
    .context("failed to read default target")?;

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
        name,
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
    branch_id: &str,
    paths: &Vec<String>,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .context("failed to get or create currnt session")?;
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .context("failed to open current session")?;

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to create branch iterator")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|branch| branch.applied)
        .collect::<Vec<_>>();

    let writer = branch::Writer::new(gb_repository);

    let mut target_branch = virtual_branches
        .iter()
        .find(|b| b.id == branch_id)
        .context("failed to find target branch")?
        .clone();

    for path in paths {
        let mut source_branch = virtual_branches
            .iter()
            .find(|b| b.ownership.contains(path))
            .context(format!("failed to find source branch for {}", path))?
            .clone();

        source_branch.ownership.retain(|f| f != path);
        writer
            .write(&source_branch)
            .context(format!("failed to write source branch for {}", path))?;

        target_branch.ownership.push(path.to_string());
        writer
            .write(&target_branch)
            .context(format!("failed to write target branch for {}", path))?;

        log::info!(
            "{}: moved file {} to branch {}",
            gb_repository.project_id,
            path,
            target_branch.name
        );
    }

    Ok(())
}

pub fn get_status_files(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Result<Vec<String>> {
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

    let diff = project_repository
        .workdir_diff(&default_target.sha)
        .context(format!(
            "failed to get diff workdir with {}",
            default_target.sha
        ))?;

    let all_files = filenames_from_diff(&diff);

    Ok(all_files)
}

fn filenames_from_diff(diff: &git2::Diff) -> Vec<String> {
    diff.deltas()
        .filter_map(|diff| diff.old_file().path().or_else(|| diff.new_file().path()))
        .map(|path| path.to_str().unwrap().to_string())
        .collect()
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

    let mut statuses = vec![];

    // find all the hunks
    let mut new_ownership = vec![];

    let mut result = HashMap::new();
    let mut results = String::new();
    let mut hunks = Vec::new();

    let mut last_path = String::new();
    let mut last_hunk_id = String::new();
    let mut hunk_numbers = String::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        if let Some(hunk) = hunk {
            hunk_numbers = format!("{}-{}", hunk.old_start(), hunk.new_start());
        }

        let new_path = delta
            .new_file()
            .path()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let hunk_id = format!("{}:{}", new_path, hunk_numbers);
        if hunk_id != last_hunk_id {
            let hunk = VirtualBranchHunk {
                id: last_hunk_id.clone(),
                name: "".to_string(),
                diff: results.clone(),
                modified_at: 0,
                file_path: last_path.clone(),
            };
            hunks.push(hunk);
            result.insert(last_path.clone(), hunks.clone());
            results = String::new();
            last_hunk_id = hunk_id;
        }
        if last_path != new_path {
            hunks = Vec::new();
            last_path = new_path;
        }

        match line.origin() {
            '+' | '-' | ' ' => results.push_str(&format!("{}", line.origin())),
            _ => {}
        }
        results.push_str(std::str::from_utf8(line.content()).unwrap());
        true
    })
    .context("failed to print diff")?;

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to read virtual branches")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<_>>();

    let all_files = filenames_from_diff(&diff);

    for file_path in &all_files {
        let mut file_found = false;
        for branch in &virtual_branches {
            for file in &branch.ownership {
                if file.eq(file_path) {
                    file_found = true;
                }
            }
        }
        if !file_found {
            new_ownership.push(file_path.clone());
        }
    }

    for branch in &virtual_branches {
        let mut files = vec![];
        if !new_ownership.is_empty() {
            // in this case, lets add any newly changed files to the first branch we see and persist it
            let mut branch = branch.clone();
            branch.ownership.extend(new_ownership.clone());
            new_ownership.clear();

            // ok, write the updated data back
            let writer = branch::Writer::new(gb_repository);
            writer.write(&branch).context("failed to write branch")?;

            for file in branch.ownership {
                if all_files.contains(&file) {
                    let filehunks = result.get(&file).unwrap();
                    let vfile = VirtualBranchFile {
                        id: file.clone(),
                        path: file.clone(),
                        hunks: filehunks.clone(),
                    };
                    // push the file to the status list
                    files.push(vfile);
                }
            }
        } else {
            for file in &branch.ownership {
                if all_files.contains(file) {
                    let filehunks = result.get(file).unwrap();
                    let vfile = VirtualBranchFile {
                        id: file.clone(),
                        path: file.clone(),
                        hunks: filehunks.clone(),
                    };
                    // push the file to the status list
                    files.push(vfile);
                }
            }
        }
        statuses.push((branch.clone(), files.clone()));
    }

    Ok(statuses)
}
