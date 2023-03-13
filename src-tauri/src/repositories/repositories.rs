use crate::{deltas, fs, projects, sessions, users};
use anyhow::{Context, Result};
use git2::{BranchType, Cred, Signature};
use std::{collections::HashMap, env, path::Path};
use tauri::regex::Regex;
use walkdir::WalkDir;

pub struct Repository {
    pub project: projects::Project,
    pub git_repository: git2::Repository,
}

impl Repository {
    pub fn open(
        projects_storage: &projects::Storage,
        users_storage: &users::Storage,
        project_id: &str,
    ) -> Result<Self> {
        let project = projects_storage
            .get_project(project_id)
            .with_context(|| "failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;
        let user = users_storage
            .get()
            .with_context(|| "failed to get user for project")?;
        let git_repository =
            git2::Repository::open(&project.path).with_context(|| "failed to open repository")?;
        Self::new(project, git_repository, user)
    }

    pub fn new(
        project: projects::Project,
        git_repository: git2::Repository,
        user: Option<users::User>,
    ) -> Result<Self> {
        init(&git_repository, &project, &user).with_context(|| "failed to init repository")?;
        Ok(Repository {
            project,
            git_repository,
        })
    }

    fn reference(&self) -> Result<git2::Reference> {
        let reference_name = self.project.refname();
        let reference = self
            .git_repository
            .find_reference(&reference_name)
            .with_context(|| {
                format!(
                    "failed to find reference {} in repository {}",
                    reference_name, self.project.path
                )
            })?;
        Ok(reference)
    }

    pub fn sessions(&self) -> Result<Vec<sessions::Session>> {
        sessions::list(&self.git_repository, &self.project, &self.reference()?)
    }

    pub fn files(
        &self,
        session_id: &str,
        files: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        sessions::list_files(
            &self.git_repository,
            &self.project,
            &self.reference()?,
            session_id,
            files,
        )
    }

    pub fn deltas(&self, session_id: &str) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        deltas::list(
            &self.git_repository,
            &self.project,
            &self.reference()?,
            session_id,
        )
    }

    // get a list of all files in the working directory
    pub fn file_paths(&self) -> Result<Vec<String>> {
        let workdir = &self
            .git_repository
            .workdir()
            .with_context(|| "failed to get working directory")?;

        let all_files = fs::list_files(&workdir)
            .with_context(|| format!("Failed to list files in {}", workdir.to_str().unwrap()))?;

        let mut files = Vec::new();
        for file in all_files {
            if !&self.git_repository.is_path_ignored(&file).unwrap_or(true) {
                files.push(file);
            }
        }
        return Ok(files);
    }

    // get a list of all files in the working directory
    pub fn match_file_paths(&self, match_pattern: &str) -> Result<Vec<String>> {
        let workdir = &self
            .git_repository
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
                                && !&self
                                    .git_repository
                                    .is_path_ignored(e.path())
                                    .unwrap_or(true))
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
        for branch in self.git_repository.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            branches.push(branch.name()?.unwrap().to_string());
        }
        Ok(branches)
    }

    // return current branch name
    pub fn branch(&self) -> Result<String> {
        print!("getting branch name... ");
        let repo = &self.git_repository;
        let head = repo.head()?;
        let branch = head.name().unwrap();
        Ok(branch.to_string())
    }

    pub fn switch_branch(&self, branch_name: &str) -> Result<bool> {
        self.flush_session(&None)
            .with_context(|| "failed to flush session before switching branch")?;

        let branch = self
            .git_repository
            .find_branch(branch_name, git2::BranchType::Local)?;
        let branch = branch.into_reference();
        self.git_repository
            .set_head(branch.name().unwrap())
            .with_context(|| "failed to set head")?;
        // checkout head
        self.git_repository
            .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))
            .with_context(|| "failed to checkout head")?;
        Ok(true)
    }

    // get file status from git
    pub fn status(&self) -> Result<HashMap<String, String>> {
        let mut options = git2::StatusOptions::new();
        options.include_untracked(true);
        options.include_ignored(false);
        options.recurse_untracked_dirs(true);

        // get the status of the repository
        let statuses = self
            .git_repository
            .statuses(Some(&mut options))
            .with_context(|| "failed to get repository status");

        let mut files = HashMap::new();

        match statuses {
            Ok(statuses) => {
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
                        _ => continue,
                    };
                    files.insert(path.to_string(), istatus.to_string());
                }
            }
            Err(_) => {
                println!("Error getting status");
            }
        }

        return Ok(files);
    }

    // commit method
    pub fn commit(&self, message: &str, files: Vec<&str>, push: bool) -> Result<bool> {
        println!("Git Commit");
        let repo = &self.git_repository;

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
        println!("Created commit {}", commit);

        if push {
            println!("Pushing to remote");

            // Get a reference to the current branch
            let head = repo.head()?;
            let branch = head.name().unwrap();

            println!("Branch: {:?}", branch);

            let branch_remote = repo.branch_upstream_remote(branch)?;
            let branch_remote_name = branch_remote.as_str().unwrap();
            let branch_name = repo.branch_upstream_name(branch)?;
            println!(
                "Branch remote: {:?}, {:?}",
                branch_remote.as_str(),
                branch_name.as_str()
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
                print!("Trying to auth with ssh... {:?} ", username_from_url);
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

        return Ok(true);
    }

    pub fn flush_session(&self, user: &Option<users::User>) -> Result<()> {
        // if the reference doesn't exist, we create it by creating a flushing a new session
        let mut current_session =
            match sessions::Session::current(&self.git_repository, &self.project)? {
                Some(session) => session,
                None => sessions::Session::from_head(&self.git_repository, &self.project)?,
            };
        current_session
            .flush(&self.git_repository, user, &self.project)
            .with_context(|| format!("{}: failed to flush session", &self.project.id))?;
        Ok(())
    }
}

fn init(
    git_repository: &git2::Repository,
    project: &projects::Project,
    user: &Option<users::User>,
) -> Result<()> {
    let reference_name = project.refname();
    match git_repository.find_reference(&reference_name) {
        // if the reference exists, we do nothing
        Ok(_) => Ok(()),
        // if the reference doesn't exist, we create it by creating a flushing a new session
        Err(error) => {
            if error.code() == git2::ErrorCode::NotFound {
                // if the reference doesn't exist, we create it by creating a flushing a new session
                let mut current_session = match sessions::Session::current(git_repository, project)?
                {
                    Some(session) => session,
                    None => sessions::Session::from_head(git_repository, project)?,
                };
                current_session
                    .flush(git_repository, user, project)
                    .with_context(|| format!("{}: failed to flush session", project.id))?;
                Ok(())
            } else {
                Err(error.into())
            }
        }
    }
}
