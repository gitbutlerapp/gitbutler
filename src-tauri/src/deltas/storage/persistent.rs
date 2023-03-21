use crate::{deltas, projects, sessions};
use anyhow::Result;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Store {
    git_repository: Arc<Mutex<git2::Repository>>,
    cache: Arc<Mutex<HashMap<String, HashMap<String, Vec<deltas::Delta>>>>>,
}

impl Store {
    pub fn new(project: projects::Project) -> Result<Self> {
        Ok(Self {
            git_repository: Arc::new(Mutex::new(git2::Repository::open(&project.path)?)),
            cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn list_all(&self, session: &sessions::Session) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        if session.hash.is_none() {
            return Err(anyhow::anyhow!(format!(
                "can not list persistent deltas from current session {}",
                session.id
            )));
        }

        let git_repository = self.git_repository.lock().unwrap();
        let commit_hash = session.hash.as_ref().unwrap();
        let commit_id = git2::Oid::from_str(commit_hash)?;
        let commit = git_repository.find_commit(commit_id)?;
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
            let blob = entry
                .to_object(&git_repository)
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

    pub fn list(
        &self,
        session: &sessions::Session,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let mut cache = self.cache.lock().unwrap();
        let all_files = match cache.get(&session.id) {
            Some(files) => files.clone(),
            None => {
                let files = self.list_all(session)?;
                cache.insert(session.id.clone(), files.clone());
                files
            }
        };

        match paths {
            Some(paths) => {
                let mut files = HashMap::new();
                for path in paths {
                    if let Some(deltas) = all_files.get(path) {
                        files.insert(path.to_owned(), deltas.clone());
                    }
                }
                Ok(files)
            }
            None => Ok(all_files),
        }
    }
}
