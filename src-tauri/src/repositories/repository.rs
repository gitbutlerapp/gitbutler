use crate::{deltas, git::activity, projects, sessions, users};
use anyhow::{Context, Result};
use git2::{BranchType, Cred, DiffOptions, Signature};
use serde::Serialize;
use std::{
    collections::HashMap,
    env,
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::regex::Regex;
use walkdir::WalkDir;

#[derive(Serialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChange,
    Other,
}

impl From<git2::Status> for FileStatus {
    fn from(status: git2::Status) -> Self {
        if status.is_index_new() || status.is_wt_new() {
            FileStatus::Added
        } else if status.is_index_modified() || status.is_wt_modified() {
            FileStatus::Modified
        } else if status.is_index_deleted() || status.is_wt_deleted() {
            FileStatus::Deleted
        } else if status.is_index_renamed() || status.is_wt_renamed() {
            FileStatus::Renamed
        } else if status.is_index_typechange() || status.is_wt_typechange() {
            FileStatus::TypeChange
        } else {
            FileStatus::Other
        }
    }
}

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

    // return current head name
    pub fn head(&self) -> Result<String> {
        let repo = self.git_repository.lock().unwrap();
        let head = repo.head()?;
        Ok(head
            .name()
            .map(|s| s.to_string())
            .unwrap_or("undefined".to_string()))
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

    pub fn activity(&self, start_time_ms: Option<u128>) -> Result<Vec<activity::Activity>> {
        let head_logs_path = Path::new(&self.project.path)
            .join(".git")
            .join("logs")
            .join("HEAD");

        if !head_logs_path.exists() {
            return Ok(Vec::new());
        }

        let activity = std::fs::read_to_string(head_logs_path)
            .with_context(|| "failed to read HEAD logs")?
            .lines()
            .filter_map(|line| activity::parse_reflog_line(line).ok())
            .collect::<Vec<activity::Activity>>();

        let activity = if let Some(start_timestamp_ms) = start_time_ms {
            activity
                .into_iter()
                .filter(|activity| activity.timestamp_ms > start_timestamp_ms)
                .collect::<Vec<activity::Activity>>()
        } else {
            activity
        };

        Ok(activity)
    }

    // returns statuses of the unstaged files in the repository
    fn unstaged_statuses(&self) -> Result<HashMap<String, FileStatus>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.recurse_untracked_dirs(true);
        options.include_ignored(false);
        options.show(git2::StatusShow::Workdir);

        let git_repository = self.git_repository.lock().unwrap();
        // get the status of the repository
        let statuses = git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status")?;

        let files = statuses
            .iter()
            .map(|entry| {
                let path = entry.path().unwrap();
                (path.to_string(), FileStatus::from(entry.status()))
            })
            .collect();

        return Ok(files);
    }

    // returns statuses of the staged files in the repository
    fn staged_statuses(&self) -> Result<HashMap<String, FileStatus>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.include_ignored(false);
        options.recurse_untracked_dirs(true);
        options.show(git2::StatusShow::Index);

        let git_repository = self.git_repository.lock().unwrap();
        // get the status of the repository
        let statuses = git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status")?;

        let files = statuses
            .iter()
            .map(|entry| {
                let path = entry.path().unwrap();
                (path.to_string(), FileStatus::from(entry.status()))
            })
            .collect();

        return Ok(files);
    }

    // get file status from git
    pub fn status(&self) -> Result<HashMap<String, (FileStatus, bool)>> {
        let staged_statuses = self.staged_statuses()?;
        let unstaged_statuses = self.unstaged_statuses()?;
        let mut statuses = HashMap::new();
        unstaged_statuses.iter().for_each(|(path, status)| {
            statuses.insert(path.clone(), (*status, false));
        });
        staged_statuses.iter().for_each(|(path, status)| {
            statuses.insert(path.clone(), (*status, true));
        });
        Ok(statuses)
    }

    // commit method
    pub fn commit(&self, message: &str, push: bool) -> Result<()> {
        let repo = self.git_repository.lock().unwrap();

        let config = repo.config().with_context(|| "failed to get config")?;
        let name = config
            .get_string("user.name")
            .with_context(|| "failed to get user.name")?;
        let email = config
            .get_string("user.email")
            .with_context(|| "failed to get user.email")?;

        // Get the default signature for the repository
        let signature = Signature::now(&name, &email).with_context(|| "failed to get signature")?;

        // Create the commit with current index
        let tree_id = repo.index()?.write_tree()?;
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

    pub fn stage_files(&self, paths: Vec<&Path>) -> Result<()> {
        let repo = self.git_repository.lock().unwrap();
        let mut index = repo.index()?;
        for path in paths {
            // to "stage" a file means to:
            // - remove it from the index if file is deleted
            // - overwrite it in the index otherwise
            if !Path::new(&self.project.path).join(path).exists() {
                index.remove_path(path).with_context(|| {
                    format!("failed to remove path {} from index", path.display())
                })?;
            } else {
                index
                    .add_path(path)
                    .with_context(|| format!("failed to add path {} to index", path.display()))?;
            }
        }
        index.write().with_context(|| "failed to write index")?;
        Ok(())
    }

    pub fn unstage_files(&self, paths: Vec<&Path>) -> Result<()> {
        let repo = self.git_repository.lock().unwrap();
        let head_tree = repo.head()?.peel_to_tree()?;
        let mut head_index = git2::Index::new()?;
        head_index.read_tree(&head_tree)?;
        let mut index = repo.index()?;
        for path in paths {
            // to "unstage" a file means to:
            // - put head version of the file in the index if it exists
            // - remove it from the index otherwise
            let head_index_entry = head_index.iter().find(|entry| {
                let entry_path = String::from_utf8(entry.path.clone());
                entry_path.as_ref().unwrap() == path.to_str().unwrap()
            });
            if let Some(entry) = head_index_entry {
                index
                    .add(&entry)
                    .with_context(|| format!("failed to add path {} to index", path.display()))?;
            } else {
                index.remove_path(path).with_context(|| {
                    format!("failed to remove path {} from index", path.display())
                })?;
            }
        }
        index.write().with_context(|| "failed to write index")?;
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
