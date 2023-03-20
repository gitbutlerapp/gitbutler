use std::{collections::HashMap, path::Path};

use crate::{projects, sessions};
use anyhow::{Context, Result};

pub struct Store {
    project: projects::Project,
    git_repository: git2::Repository,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            git_repository: git2::Repository::open(&self.project.path).unwrap(),
        }
    }
}

impl Store {
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Result<Self> {
        Ok(Self {
            project: project.clone(),
            git_repository,
        })
    }

    pub fn get_by_id(&self, session_id: &str) -> Result<Option<sessions::Session>> {
        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let head = self
            .git_repository
            .find_commit(reference.target().unwrap())?;
        let mut walker = self.git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TIME)?;

        for commit_id in walker {
            let commit = self.git_repository.find_commit(commit_id?)?;
            if sessions::id_from_commit(&self.git_repository, &commit)? == session_id {
                return Ok(Some(sessions::Session::from_commit(
                    &self.git_repository,
                    &commit,
                )?));
            }
        }

        Ok(None)
    }

    pub fn list_files(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        let files = self.list_files_from_disk(session_id)?;
        match paths {
            Some(paths) => {
                let mut filtered_files = HashMap::new();
                for path in paths {
                    if let Some(file) = files.get(path) {
                        filtered_files.insert(path.to_string(), file.to_string());
                    }
                }
                Ok(filtered_files)
            }
            None => Ok(files),
        }
    }

    fn list_files_from_disk(&self, session_id: &str) -> Result<HashMap<String, String>> {
        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let commit = if is_current_session_id(&self.project, session_id)? {
            let head_commit = reference.peel_to_commit()?;
            Some(head_commit)
        } else {
            let head_commit = reference.peel_to_commit()?;
            let mut walker = self.git_repository.revwalk()?;
            walker.push(head_commit.id())?;
            walker.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

            let mut session_commit = None;
            let mut previous_session_commit = None;
            for commit_id in walker {
                let commit = self.git_repository.find_commit(commit_id?)?;
                if sessions::id_from_commit(&self.git_repository, &commit)? == session_id {
                    session_commit = Some(commit);
                    break;
                }
                previous_session_commit = Some(commit.clone());
            }

            match (previous_session_commit, session_commit) {
                // if there is a previous session, we want to list the files from the previous session
                (Some(previous_session_commit), Some(_)) => Some(previous_session_commit),
                // if there is no previous session, we use the found session, because it's the first one.
                (None, Some(session_commit)) => Some(session_commit),
                _ => None,
            }
        };

        if commit.is_none() {
            return Ok(HashMap::new());
        }
        let commit = commit.unwrap();

        let tree = commit.tree()?;
        let mut files = HashMap::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.name().is_none() {
                return git2::TreeWalkResult::Ok;
            }
            let entry_path = Path::new(root).join(entry.name().unwrap());
            if !entry_path.starts_with("wd") {
                return git2::TreeWalkResult::Ok;
            }
            if "wd".eq(entry_path.to_str().unwrap()) {
                return git2::TreeWalkResult::Ok;
            }

            if entry.kind() == Some(git2::ObjectType::Tree) {
                return git2::TreeWalkResult::Ok;
            }

            let blob = entry
                .to_object(&self.git_repository)
                .and_then(|obj| obj.peel_to_blob());
            let content = blob.map(|blob| blob.content().to_vec());

            let relpath = entry_path.strip_prefix("wd").unwrap();

            files.insert(
                relpath.to_owned().to_str().unwrap().to_owned(),
                String::from_utf8(content.unwrap_or_default()).unwrap_or_default(),
            );

            git2::TreeWalkResult::Ok
        })?;

        Ok(files)
    }

    // returns list of sessions in reverse chronological order
    // except for the first session. The first created session
    // is special and used to bootstrap the gitbutler state inside a repo.
    // see crate::repositories::inib
    pub fn list(&self, earliest_timestamp_ms: Option<u128>) -> Result<Vec<sessions::Session>> {
        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let head = self
            .git_repository
            .find_commit(reference.target().unwrap())?;

        // list all commits from gitbutler head to the first commit
        let mut walker = self.git_repository.revwalk()?;
        walker.push(head.id())?;
        walker.set_sorting(git2::Sort::TOPOLOGICAL)?;

        let mut sessions: Vec<sessions::Session> = vec![];
        for id in walker {
            let id = id?;
            let commit = self.git_repository.find_commit(id).with_context(|| {
                format!(
                    "failed to find commit {} in repository {}",
                    id.to_string(),
                    self.git_repository.path().display()
                )
            })?;
            let session = sessions::Session::from_commit(&self.git_repository, &commit)?;
            match earliest_timestamp_ms {
                Some(earliest_timestamp_ms) => {
                    if session.meta.start_timestamp_ms <= earliest_timestamp_ms {
                        break;
                    }
                }
                None => {}
            }
            sessions.push(session);
        }

        // drop the first session, which is the bootstrap session
        sessions.pop();

        Ok(sessions)
    }
}

fn is_current_session_id(project: &projects::Project, session_id: &str) -> Result<bool> {
    let current_id_path = project.session_path().join("meta").join("id");
    if !current_id_path.exists() {
        return Ok(false);
    }
    let current_id = std::fs::read_to_string(current_id_path)?;
    return Ok(current_id == session_id);
}
