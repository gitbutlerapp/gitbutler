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
    pub kind: String,
    pub files: Vec<VirtualBranchFile>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchFile {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub hunks: Vec<VirtualBranchHunk>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchHunk {
    pub id: String,
    pub name: String,
    pub diff: String,
    pub kind: String,
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
            kind: "branch".to_string(),
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
    let commit = repo.find_commit(default_target.sha).unwrap();
    let tree = commit.tree().unwrap();

    let now = time::UNIX_EPOCH.elapsed().unwrap().as_millis();

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
    writer.write(&branch).unwrap();
    Ok(branch.id)
}

pub fn move_files(
    gb_repository: &gb_repository::Repository,
    branch_id: String,
    paths: Vec<String>,
) -> Result<()> {
    let current_session = gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(gb_repository, &current_session)
        .expect("failed to open current session reader");

    let virtual_branches = Iterator::new(&current_session_reader)
        .expect("failed to read virtual branches")
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .expect("failed to read virtual branches")
        .into_iter()
        .collect::<Vec<_>>();

    // rewrite ownership of both branches
    let writer = branch::Writer::new(gb_repository);
    for mut branch in virtual_branches {
        if branch.id == branch_id {
            branch.ownership.extend(paths.iter().map(|f| f.to_string()));
        } else {
            branch.ownership.retain(|f| !paths.contains(f));
        }
        writer.write(&branch).unwrap();
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

    let repo = &project_repository.git_repository;
    let commit = repo.find_commit(default_target.sha).unwrap();
    let tree = commit.tree().unwrap();

    // list the files that are different between the wd and the base sha
    let mut opts = git2::DiffOptions::new();
    opts.recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_untracked_content(true);
    let diff = repo
        .diff_tree_to_workdir(Some(&tree), Some(&mut opts))
        .unwrap();

    let mut all_files = vec![];

    let deltas = diff.deltas();
    for delta in deltas {
        let mut file_path = "".to_string();
        let old_file = delta.old_file();
        let new_file = delta.new_file();

        if let Some(path) = new_file.path() {
            file_path = path.to_str().unwrap().to_string();
        } else if let Some(path) = old_file.path() {
            file_path = path.to_str().unwrap().to_string();
        }
        all_files.push(file_path.clone());
    }
    Ok(all_files)
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

    let mut statuses = vec![];

    //println!("  base sha: {}", sha.blue());

    let repo = &project_repository.git_repository;
    let commit = repo.find_commit(default_target.sha).unwrap();
    let tree = commit.tree().unwrap();

    // list the files that are different between the wd and the base sha
    let mut opts = git2::DiffOptions::new();
    opts.recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_untracked_content(true);
    let diff = repo
        .diff_tree_to_workdir(Some(&tree), Some(&mut opts))
        .unwrap();

    // find all the hunks
    let mut all_files = vec![];
    let mut new_ownership = vec![];

    let mut result = HashMap::new();
    let mut results = String::new();
    let mut hunks = Vec::new();

    let mut current_line_count = 0;
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

        let mut hunk_id = new_path.clone();
        hunk_id.push(':');
        hunk_id.push_str(&hunk_numbers);

        if hunk_id != last_hunk_id {
            let hunk = VirtualBranchHunk {
                id: last_hunk_id.clone(),
                name: "".to_string(),
                diff: results.clone(),
                kind: "hunk".to_string(),
                modified_at: 0,
                file_path: last_path.clone(),
            };
            hunks.push(hunk);
            result.insert(last_path.clone(), hunks.clone());
            results = String::new();
            current_line_count = 0;
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
        current_line_count += 1;
        true
    })
    .unwrap();

    let virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to read virtual branches")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<_>>();

    let deltas = diff.deltas();
    for delta in deltas {
        let mut file_path = "".to_string();
        let old_file = delta.old_file();
        let new_file = delta.new_file();

        if let Some(path) = new_file.path() {
            file_path = path.to_str().unwrap().to_string();
        } else if let Some(path) = old_file.path() {
            file_path = path.to_str().unwrap().to_string();
        }
        all_files.push(file_path.clone());

        let mut file_found = false;
        for branch in &virtual_branches {
            for file in &branch.ownership {
                if *file == file_path {
                    file_found = true;
                }
            }
        }
        if !file_found {
            new_ownership.push(file_path.clone());
        }
    }

    //println!("new ownership: {:?}", new_ownership);
    //println!("sha: {}", sha);
    //println!("all files: {:?}", all_files);

    for branch in &virtual_branches {
        let mut files = vec![];
        if !new_ownership.is_empty() {
            // in this case, lets add any newly changed files to the first branch we see and persist it
            let mut branch = branch.clone();
            branch.ownership.extend(new_ownership.clone());
            new_ownership.clear();

            // ok, write the updated data back
            let writer = branch::Writer::new(gb_repository);
            writer.write(&branch).unwrap();

            for file in branch.ownership {
                if all_files.contains(&file) {
                    let filehunks = result.get(&file).unwrap();
                    let vfile = VirtualBranchFile {
                        id: file.clone(),
                        path: file.clone(),
                        kind: "file".to_string(),
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
                        kind: "file".to_string(),
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
