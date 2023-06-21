pub mod branch;
mod iterator;
pub mod target;

use std::{collections::HashMap, time, vec};

use anyhow::{Context, Result};
use filetime::FileTime;
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
            .find(|b| {
                b.ownership
                    .iter()
                    .map(|o| o.file_path.display().to_string())
                    .collect::<Vec<_>>()
                    .contains(path)
            })
            .context(format!("failed to find source branch for {}", path))?
            .clone();

        source_branch
            .ownership
            .retain(|f| !f.file_path.display().to_string().eq(path));
        source_branch.ownership.sort();
        source_branch.ownership.dedup();

        writer
            .write(&source_branch)
            .context(format!("failed to write source branch for {}", path))?;

        target_branch.ownership.push(path.into());
        target_branch.ownership.sort();
        target_branch.ownership.dedup();

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
    let mut mtimes = HashMap::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        if let Some(hunk) = hunk {
            hunk_numbers = format!("{}-{}", hunk.old_start(), hunk.new_start());

            let new_path = delta
                .new_file()
                .path()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let mtime = match mtimes.get(&new_path) {
                Some(mtime) => *mtime,
                None => {
                    let file_path = project_repository
                        .git_repository
                        .workdir()
                        .unwrap()
                        .join(new_path.clone());

                    let metadata = file_path.metadata().unwrap();
                    let mtime = FileTime::from_last_modification_time(&metadata);
                    // convert seconds and nanoseconds to milliseconds
                    let mtime = (mtime.seconds() as u128 * 1000) as u128;
                    mtimes.insert(new_path.clone(), mtime);
                    mtime
                }
            };

            let hunk_id = format!("{}:{}", new_path, hunk_numbers);
            if hunk_id != last_hunk_id {
                let hunk = VirtualBranchHunk {
                    id: last_hunk_id.clone(),
                    name: "".to_string(),
                    diff: results.clone(),
                    modified_at: mtime,
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
        }
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
            for ownership in &branch.ownership {
                if ownership.file_path.display().to_string().eq(file_path) {
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
            branch
                .ownership
                .extend(new_ownership.iter().map(|file| branch::Ownership {
                    file_path: file.into(),
                    ranges: vec![],
                }));
            new_ownership.clear();

            // ok, write the updated data back
            let writer = branch::Writer::new(gb_repository);
            writer.write(&branch).context("failed to write branch")?;

            for file in branch.ownership {
                let file = file.file_path.display().to_string();
                if all_files.contains(&file) {
                    let filehunks = result.get(&file).cloned().unwrap_or(Vec::new());
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
                let file = file.file_path.display().to_string();
                if all_files.contains(&file) {
                    match result.get(&file) {
                        Some(filehunks) => {
                            let vfile = VirtualBranchFile {
                                id: file.clone(),
                                path: file.clone(),
                                hunks: filehunks.clone(),
                            };
                            files.push(vfile);
                        }
                        // push the file to the status list
                        None => {
                            println!("  no hunks for {}", file);
                            continue;
                        }
                    }
                }
            }
        }
        statuses.push((branch.clone(), files.clone()));
    }

    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{projects, storage, users};

    use super::*;

    static mut TEST_TARGET_INDEX: usize = 0;

    fn test_target() -> target::Target {
        target::Target {
            name: format!("target_name_{}", unsafe { TEST_TARGET_INDEX }),
            remote: format!("remote_{}", unsafe { TEST_TARGET_INDEX }),
            sha: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                unsafe { TEST_TARGET_INDEX }
            ))
            .unwrap(),
        }
    }

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> branch::Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        branch::Branch {
            id: format!("branch_{}", unsafe { TEST_INDEX }),
            name: format!("branch_name_{}", unsafe { TEST_INDEX }),
            applied: true,
            upstream: format!("upstream_{}", unsafe { TEST_INDEX }),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
            head: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef0123456{}",
                unsafe { TEST_INDEX }
            ))
            .unwrap(),
            tree: git2::Oid::from_str(&format!(
                "0123456789abcdef0123456789abcdef012345{}",
                unsafe { TEST_INDEX + 10 }
            ))
            .unwrap(),
            ownership: vec![branch::Ownership {
                file_path: format!("file/{}", unsafe { TEST_INDEX }).into(),
                ranges: vec![],
            }],
        }
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
        std::fs::write(std::path::Path::new(&project.path).join(file_path), "test")?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        let status =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let branch_ids = status
            .iter()
            .map(|(branch, _)| branch.id.clone())
            .collect::<Vec<_>>();
        let all_files = status
            .iter()
            .flat_map(|(_, files)| files.iter().map(|f| f.path.clone()))
            .collect::<Vec<_>>();

        assert_eq!(status.len(), 2);
        assert_eq!(branch_ids.len(), 2);
        assert!(branch_ids.contains(&branch1_id));
        assert!(branch_ids.contains(&branch2_id));
        assert_eq!(all_files.len(), 1);
        assert_eq!(all_files[0], file_path.to_str().unwrap());

        Ok(())
    }
}
