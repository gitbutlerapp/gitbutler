pub mod branch;
mod iterator;
pub mod target;
use serde::Serialize;

use std::{collections::HashMap, vec};

use crate::{gb_repository, project_repository};
pub use branch::Branch;
pub use iterator::BranchIterator as Iterator;
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
) -> Vec<VirtualBranch> {
    let mut branches: Vec<VirtualBranch> = Vec::new();

    let statuses = get_status_by_branch(&gb_repository, &project_repository);
    for (branch_id, files) in statuses {
        let branch = gb_repository.get_virtual_branch(&branch_id).unwrap();
        let mut vfiles = vec![];
        for file in files {
            vfiles.push(file);
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
    branches
}

pub fn get_status_files(
    gb_repository: &gb_repository::Repository,
    project_repository: &project_repository::Repository,
) -> Vec<String> {
    if let Some(sha) = get_base_sha(gb_repository) {
        let repo = &project_repository.git_repository;
        let oid = git2::Oid::from_str(&sha).unwrap();
        let commit = repo.find_commit(oid).unwrap();
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
        all_files
    } else {
        vec![]
    }
}

pub fn get_base_sha(gb_repository: &gb_repository::Repository) -> Option<String> {
    let reader = gb_repository.get_branch_dir_reader();
    let target_reader = target::Reader::new(&reader);
    if let Ok(target) = target_reader.read_default() {
        Some(target.sha.to_string())
    } else {
        None
    }
}

// list the virtual branches and their file statuses (statusi?)
pub fn get_status_by_branch<'a>(
    gb_repository: &'a gb_repository::Repository,
    project_repository: &'a project_repository::Repository<'a>,
) -> Vec<(String, Vec<VirtualBranchFile>)> {
    let mut statuses = vec![];

    if let Some(sha) = get_base_sha(gb_repository) {
        //println!("  base sha: {}", sha.blue());
        let branch_reader = gb_repository.get_branch_dir_reader();

        let repo = &project_repository.git_repository;
        let oid = git2::Oid::from_str(&sha).unwrap();
        let commit = repo.find_commit(oid).unwrap();
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
            hunk_id.push_str(":");
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

            let mut branch_iter = Iterator::new(&branch_reader).unwrap();
            let mut file_found = false;
            while let Some(item) = branch_iter.next() {
                if let Ok(item) = item {
                    for file in item.ownership {
                        if file == file_path {
                            file_found = true;
                        }
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

        let _vbranch_reader = branch::Reader::new(&branch_reader);
        let mut iter = Iterator::new(&branch_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(branch) = item {
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
                }
                statuses.push((branch.id.clone(), files.clone()));
            }
        }
    } else {
        println!("  no base sha set, run butler setup");
    }

    statuses
}
