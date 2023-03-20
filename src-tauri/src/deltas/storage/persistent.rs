use crate::{deltas, projects, sessions};
use anyhow::Result;
use std::{collections::HashMap, path::Path};

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
    pub fn new(project: projects::Project) -> Result<Self> {
        Ok(Self {
            git_repository: git2::Repository::open(&project.path)?,
            project,
        })
    }

    pub fn list(
        &self,
        session: &sessions::Session,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        if session.hash.is_none() {
            return Err(anyhow::anyhow!(format!(
                "can not list persistent deltas from current session {}",
                session.id
            )));
        }

        let commit_hash = session.hash.as_ref().unwrap();
        let commit_id = git2::Oid::from_str(commit_hash)?;
        let commit = self.git_repository.find_commit(commit_id)?;
        let tree = commit.tree()?;

        let mut blobs = HashMap::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.name().is_none() {
                return git2::TreeWalkResult::Ok;
            }
            let entry_path = Path::new(root).join(entry.name().unwrap());
            if !entry_path.starts_with("session/deltas") {
                return git2::TreeWalkResult::Ok;
            }
            if entry.kind() != Some(git2::ObjectType::Blob) {
                return git2::TreeWalkResult::Ok;
            }

            let relative_file_path = entry_path.strip_prefix("session/deltas").unwrap();
            if let Some(paths) = &paths {
                if !paths.contains(&relative_file_path.to_str().unwrap()) {
                    return git2::TreeWalkResult::Ok;
                }
            }

            let blob = entry
                .to_object(&self.git_repository)
                .and_then(|obj| obj.peel_to_blob());
            let content = blob.map(|blob| blob.content().to_vec());

            match content {
                Ok(content) => {
                    let deltas: Result<Vec<deltas::Delta>> =
                        serde_json::from_slice(&content).map_err(|e| e.into());
                    blobs.insert(relative_file_path.to_owned(), deltas);
                }
                Err(e) => {
                    log::error!("Could not get blob for {}: {:#}", entry_path.display(), e);
                }
            }
            git2::TreeWalkResult::Ok
        })?;

        let deltas = blobs
            .into_iter()
            .map(|(path, deltas)| (path.to_str().unwrap().to_owned(), deltas.unwrap()))
            .collect();
        Ok(deltas)
    }
}
