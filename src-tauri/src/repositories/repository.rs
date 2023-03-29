use crate::{deltas, fs, projects, sessions, users};
use anyhow::{Context, Result};
use git2::{BranchType, Cred, DiffOptions, Signature};
use std::{
    collections::HashMap,
    env, fs as std_fs,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::regex::Regex;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct Repository {
    pub project: projects::Project,
    pub git_repository: Arc<Mutex<git2::Repository>>,
    pub deltas_storage: deltas::Store,
    pub sessions_storage: sessions::Store,
}

impl Repository {
    pub fn new(project: projects::Project, user: Option<users::User>) -> Result<Self> {
        let git_repository = Arc::new(Mutex::new(git2::Repository::open(&project.path)?));
        let sessions_storage = sessions::Store::new(git_repository.clone(), project.clone());
        init(
            git2::Repository::open(&project.path)?,
            &project,
            user,
            &sessions_storage,
        )
        .with_context(|| "failed to init repository")?;
        Ok(Repository {
            project: project.clone(),
            deltas_storage: deltas::Store::new(
                git_repository.clone(),
                project,
                sessions_storage.clone(),
            ),
            git_repository,
            sessions_storage,
        })
    }

    pub fn sessions(&self, earliest_timestamp_ms: Option<u128>) -> Result<Vec<sessions::Session>> {
        self.sessions_storage.list(earliest_timestamp_ms)
    }

    pub fn files(
        &self,
        session_id: &str,
        files: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        self.sessions_storage.list_files(session_id, files)
    }

    pub fn deltas(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        self.deltas_storage.list(session_id, paths)
    }

    // get a list of all files in the working directory
    pub fn match_file_paths(&self, match_pattern: &str) -> Result<Vec<String>> {
        let repo = self.git_repository.lock().unwrap();
        let workdir = repo
            .workdir()
            .with_context(|| "failed to get working directory")?;

        let pattern = Regex::new(match_pattern).with_context(|| "regex parse error");
        match pattern {
            Ok(pattern) => {
                let mut files = vec![];
                for entry in WalkDir::new(workdir)
                    .into_iter()
                    .filter_entry(|e| {
                        // need to remove workdir so we're not matching it
                        let match_string = e
                            .path()
                            .strip_prefix::<&Path>(workdir.as_ref())
                            .unwrap()
                            .to_str()
                            .unwrap();
                        // this is to make it faster, so we dont have to traverse every directory if it is ignored by git
                        e.path().to_str() == workdir.to_str()  // but we need to traverse the first one
                            || ((e.file_type().is_dir() // traverse all directories if they are not ignored by git
                                || pattern.is_match(match_string)) // but only pass on files that match the regex
                                && !repo.is_path_ignored(&e.path()).unwrap_or(true))
                    })
                    .filter_map(Result::ok)
                {
                    if entry.file_type().is_file() {
                        // only save the matching files, not the directories
                        let path = entry.path();
                        let path =
                            path.strip_prefix::<&Path>(workdir.as_ref())
                                .with_context(|| {
                                    format!(
                                        "failed to strip prefix from path {}",
                                        path.to_str().unwrap()
                                    )
                                })?;
                        let path = path.to_str().unwrap().to_string();
                        files.push(path);
                    }
                }
                files.sort();
                return Ok(files);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    pub fn branches(&self) -> Result<Vec<String>> {
        let mut branches = vec![];
        let repo = self.git_repository.lock().unwrap();
        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            branches.push(branch.name()?.unwrap().to_string());
        }
        Ok(branches)
    }

    // return current branch name
    pub fn branch(&self) -> Result<String> {
        let repo = self.git_repository.lock().unwrap();
        let head = repo.head()?;
        let branch = head.name().unwrap();
        Ok(branch.to_string())
    }

    // return file contents for path in the working directory
    pub fn get_file_contents(&self, path: &str) -> Result<String> {
        let repo = self.git_repository.lock().unwrap();
        let workdir = repo
            .workdir()
            .with_context(|| "failed to get working directory")?;

        let file_path = workdir.join(path);
        // read the file contents
        let content =
            std_fs::read_to_string(file_path).with_context(|| "failed to read file contents")?;
        Ok(content)
    }

    pub fn wd_diff(&self, max_lines: usize) -> Result<HashMap<String, String>> {
        let repo = self.git_repository.lock().unwrap();
        let head = repo.head()?;
        let tree = head.peel_to_tree()?;

        // Prepare our diff options based on the arguments given
        let mut opts = DiffOptions::new();
        opts.recurse_untracked_dirs(true)
            .include_untracked(true)
            .include_ignored(true);

        let diff = repo.diff_tree_to_workdir(Some(&tree), Some(&mut opts))?;

        let mut result = HashMap::new();
        let mut results = String::new();

        let mut current_line_count = 0;
        let mut last_path = String::new();

        diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
            let new_path = delta
                .new_file()
                .path()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            print!(
                "{} {}",
                new_path,
                std::str::from_utf8(line.content()).unwrap()
            );
            if new_path != last_path {
                result.insert(last_path.clone(), results.clone());
                results = String::new();
                current_line_count = 0;
                last_path = new_path.clone();
            }
            if current_line_count <= max_lines {
                match line.origin() {
                    '+' | '-' | ' ' => results.push_str(&format!("{}", line.origin())),
                    _ => {}
                }
                results.push_str(&format!("{}", std::str::from_utf8(line.content()).unwrap()));
                current_line_count += 1;
            }
            true
        })?;
        result.insert(last_path.clone(), results.clone());
        Ok(result)
    }

    pub fn switch_branch(&self, branch_name: &str) -> Result<bool> {
        self.flush_session(None)
            .with_context(|| "failed to flush session before switching branch")?;

        let repo = self.git_repository.lock().unwrap();

        let branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
        let branch = branch.into_reference();
        repo.set_head(branch.name().unwrap())
            .with_context(|| "failed to set head")?;
        // checkout head
        repo.checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))
            .with_context(|| "failed to checkout head")?;
        Ok(true)
    }

    // get file status from git
    pub fn status(&self) -> Result<HashMap<String, String>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.include_ignored(false);
        options.recurse_untracked_dirs(true);

        let git_repository = self.git_repository.lock().unwrap();
        // get the status of the repository
        let statuses = git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status")?;

        let mut files = HashMap::new();

        // iterate over the statuses
        for entry in statuses.iter() {
            // get the path of the entry
            let path = entry.path().unwrap();
            // get the status as a string
            let istatus = match entry.status() {
                s if s.contains(git2::Status::WT_NEW) => "added",
                s if s.contains(git2::Status::WT_MODIFIED) => "modified",
                s if s.contains(git2::Status::WT_DELETED) => "deleted",
                s if s.contains(git2::Status::WT_RENAMED) => "renamed",
                s if s.contains(git2::Status::WT_TYPECHANGE) => "typechange",
                s if s.contains(git2::Status::INDEX_NEW) => "added",
                s if s.contains(git2::Status::INDEX_MODIFIED) => "modified",
                s if s.contains(git2::Status::INDEX_DELETED) => "deleted",
                s if s.contains(git2::Status::INDEX_RENAMED) => "renamed",
                s if s.contains(git2::Status::INDEX_TYPECHANGE) => "typechange",
                _ => "other",
            };
            files.insert(path.to_string(), istatus.to_string());
        }

        return Ok(files);
    }

    // commit method
    pub fn commit(&self, message: &str, files: Vec<&str>, push: bool) -> Result<()> {
        let repo = self.git_repository.lock().unwrap();

        let config = repo.config()?;
        let name = config.get_string("user.name")?;
        let email = config.get_string("user.email")?;

        // Get the repository's index
        let mut index = repo.index()?;

        // Add the specified files to the index
        for path_str in files {
            let path = Path::new(path_str);
            index.add_path(path)?;
        }

        // Write the updated index to disk
        index.write()?;

        // Get the default signature for the repository
        let signature = Signature::now(&name, &email)?;

        // Create the commit with the updated index
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let parent_commit = repo.head()?.peel_to_commit()?;
        let commit = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;
        log::info!("{}: created commit {}", self.project.id, commit);

        if push {
            // Get a reference to the current branch
            let head = repo.head()?;
            let branch = head.name().unwrap();

            let branch_remote = repo.branch_upstream_remote(branch)?;
            let branch_remote_name = branch_remote.as_str().unwrap();
            let branch_name = repo.branch_upstream_name(branch)?;

            log::info!(
                "{}: pushing {} to {} as {}",
                self.project.id,
                branch,
                branch_remote_name,
                branch_name.as_str().unwrap()
            );

            // Set the remote's callbacks
            let mut callbacks = git2::RemoteCallbacks::new();

            callbacks.push_update_reference(move |refname, message| {
                log::info!("pushing reference '{}': {:?}", refname, message);
                Ok(())
            });
            callbacks.push_transfer_progress(move |one, two, three| {
                log::info!("transferred {}/{}/{} objects", one, two, three);
            });

            // create ssh key if it's not there

            // try to auth with creds from an ssh-agent
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                Cred::ssh_key(
                    username_from_url.unwrap(),
                    None,
                    std::path::Path::new(&format!("{}/.ssh/id_ed25519", env::var("HOME").unwrap())),
                    None,
                )
            });

            let mut push_options = git2::PushOptions::new();
            push_options.remote_callbacks(callbacks);

            // Push to the remote
            let mut remote = repo.find_remote(branch_remote_name)?;
            remote
                .push(&[branch], Some(&mut push_options))
                .with_context(|| {
                    format!("failed to push {:?} to {:?}", branch, branch_remote_name)
                })?;
        }

        return Ok(());
    }

    pub fn flush_session(&self, user: Option<users::User>) -> Result<()> {
        // if the reference doesn't exist, we create it by creating a flushing a new session
        let current_session = match self.sessions_storage.get_current()? {
            Some(session) => session,
            None => self.sessions_storage.create_current()?,
        };
        self.sessions_storage
            .flush(&current_session, user)
            .with_context(|| format!("{}: failed to flush session", &self.project.id))?;
        Ok(())
    }
}

fn init(
    git_repository: git2::Repository,
    project: &projects::Project,
    user: Option<users::User>,
    sessions_storage: &sessions::Store,
) -> Result<()> {
    let reference = git_repository.find_reference(&project.refname());
    match reference {
        // if the reference exists, we do nothing
        Ok(_) => Ok(()),
        // if the reference doesn't exist, we create it by creating a flushing a new session
        Err(error) => {
            if error.code() == git2::ErrorCode::NotFound {
                // if the reference doesn't exist, we create it by creating a flushing a new session
                let current_session = match sessions_storage.get_current()? {
                    Some(session) => session,
                    None => sessions_storage.create_current()?,
                };
                sessions_storage
                    .flush(&current_session, user)
                    .with_context(|| format!("{}: failed to flush session", project.id))?;
                Ok(())
            } else {
                Err(error.into())
            }
        }
    }
}
