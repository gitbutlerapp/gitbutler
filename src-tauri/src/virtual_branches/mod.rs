pub mod branch;
mod iterator;
pub mod target;

use std::{collections::HashMap, path, time, vec};

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
    dst_branch_id: &str,
    to_move: &Vec<branch::Ownership>,
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
        .find(|b| b.id == dst_branch_id)
        .context("failed to find target branch")?
        .clone();

    for ownership in to_move {
        let mut source_branch = virtual_branches
            .iter()
            .find(|b| b.ownership.contains(ownership))
            .context(format!("failed to find source branch for {}", ownership))?
            .clone();

        source_branch.ownership.retain(|o| !o.eq(ownership));
        source_branch.ownership.sort();
        source_branch.ownership.dedup();

        writer
            .write(&source_branch)
            .context(format!("failed to write source branch for {}", ownership))?;

        target_branch.ownership.push(ownership.clone());
        target_branch.ownership.sort();
        target_branch.ownership.dedup();

        writer
            .write(&target_branch)
            .context(format!("failed to write target branch for {}", ownership))?;

        log::info!(
            "{}: moved file {} to branch {}",
            gb_repository.project_id,
            ownership,
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
                let metadata = file_path.metadata().unwrap();
                let mtime = FileTime::from_last_modification_time(&metadata);
                // convert seconds and nanoseconds to milliseconds
                let mtime = mtime.seconds() as u128 * 1000;
                mtimes.insert(file_path, mtime);
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

        if is_hunk_changed {
            let file_path = file_path.to_str().unwrap().to_string();
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
        }

        if is_path_changed {
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

    let mut virtual_branches = Iterator::new(&current_session_reader)
        .context("failed to read virtual branches")?
        .collect::<Result<Vec<branch::Branch>, reader::Error>>()
        .context("failed to read virtual branches")?;
    // sort by created timestamp so that default selected branch is the earliest created one
    virtual_branches.sort_by(|a, b| a.created_timestamp_ms.cmp(&b.created_timestamp_ms));
    let first_applied_id = virtual_branches
        .iter()
        .find(|b| b.applied)
        .map(|b| b.id.clone());
    let branch_reader = branch::Reader::new(&current_session_reader);
    let default_branch_id = branch_reader
        .read_selected()
        .context("failed to read selected branch")?
        .or(first_applied_id);

    let all_files = hunks_by_filepath.keys().cloned().collect::<Vec<_>>();

    let not_yet_owned_files = all_files
        .iter()
        .filter(|file| {
            !virtual_branches.iter().any(|branch| {
                branch
                    .ownership
                    .iter()
                    .any(|ownership| ownership.file_path.display().to_string().eq(*file))
            })
        })
        .collect::<Vec<_>>();

    if !not_yet_owned_files.is_empty() && default_branch_id.is_some() {
        let mut default_branch = virtual_branches
            .iter()
            .find(|b| b.id.eq(default_branch_id.as_ref().unwrap()))
            .unwrap()
            .clone();

        // in this case, lets add any newly changed files to the first branch we see and persist it
        default_branch
            .ownership
            .extend(not_yet_owned_files.iter().map(|file| branch::Ownership {
                file_path: file.into(),
                ranges: vec![],
            }));

        // ok, write the updated data back
        let writer = branch::Writer::new(gb_repository);
        writer
            .write(&default_branch)
            .context("failed to write branch")?;

        // update the virtual branches
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
    }

    for branch in &virtual_branches {
        let mut files = vec![];
        for file in &branch.ownership {
            let file = file.file_path.display().to_string();
            if all_files.contains(&file) {
                match hunks_by_filepath.get(&file) {
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
                        continue;
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
        std::fs::write(
            std::path::Path::new(&project.path).join(file_path),
            "line1\nline2\n",
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();
        let all_files = files_by_branch_id
            .values()
            .flat_map(|files| files.iter())
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert!(files_by_branch_id.contains_key(&branch1_id));
        assert!(files_by_branch_id.contains_key(&branch2_id));
        assert_eq!(all_files.len(), 1);
        assert!(all_files.contains(&file_path.to_str().unwrap().to_string()));

        Ok(())
    }

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

    #[test]
    fn test_get_correct_diff_displayed() -> Result<()> {
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
        let file_content_before = r#"
            line1
            line2
            line3
            line4
            line5
            line6
            line7
            line8
            line9
            line10 
        "#;
        std::fs::write(
            repository.path().parent().unwrap().join("test.txt"),
            // std::path::Path::new(&project.path).join(file_path),
            file_content_before,
        )?;


        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "foo".to_string(),
            remote: "foo".to_string(),
            sha: repository.head().unwrap().target().unwrap(),
        })?;

        let commit_id = commit_all(&repository)?;
        assert!(repository.find_commit(commit_id).is_ok());


        // Update target???????

        target::Writer::new(&gb_repo).write_default(&target::Target {
            name: "foo".to_string(),
            remote: "foo".to_string(),
            sha: commit_id,
        })?;


        let file_content_after = r#"
            line1-changed
            line2
            line3
            line4
            line5
            line6
            line7
            line8
            line9
            line10-changed
        "#;

        std::fs::write(
            // file_path,
            repository.path().parent().unwrap().join("test.txt"),
            file_content_after,
        )?;

        let branch1_id = create_virtual_branch(&gb_repo, "test_branch")
            .expect("failed to create virtual branch");
        let branch2_id = create_virtual_branch(&gb_repo, "test_branch2")
            .expect("failed to create virtual branch");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");
        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();
        let all_files = files_by_branch_id
            .values()
            .flat_map(|files| files.iter())
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert!(files_by_branch_id.contains_key(&branch1_id));
        assert!(files_by_branch_id.contains_key(&branch2_id));
        assert_eq!(all_files.len(), 1);
        assert!(all_files.contains(&file_path.to_str().unwrap().to_string()));

        // Iterate over hunks of files_by_branch_id
        let hunks = files_by_branch_id
            .get(&branch1_id)
            .unwrap()
            .iter()
            .flat_map(|file| file.hunks.iter())
            .collect::<Vec<_>>();

        // assert the nuber of hunks is 2
        // assert_eq!(hunks.len(), 1);

        for hunk in hunks {
            print!("{}", hunk.diff);
        }

        Ok(())
    }

    #[test]
    fn test_move_files() -> Result<()> {
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

        move_files(
            &gb_repo,
            &branch2_id,
            &vec![file_path.to_str().unwrap().into()],
        )
        .expect("failed to move files");

        let statuses =
            get_status_by_branch(&gb_repo, &project_repository).expect("failed to get status");

        let files_by_branch_id = statuses
            .iter()
            .map(|(branch, files)| (branch.id.clone(), files))
            .collect::<HashMap<_, _>>();

        assert_eq!(files_by_branch_id.len(), 2);
        assert_eq!(files_by_branch_id[&branch1_id].len(), 0);
        assert_eq!(files_by_branch_id[&branch2_id].len(), 1);

        Ok(())
    }
}
