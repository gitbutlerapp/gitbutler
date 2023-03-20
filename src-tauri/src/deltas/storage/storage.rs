use crate::{deltas, fs, projects, sessions};
use anyhow::{anyhow, Context, Result};
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
    pub fn new(git_repository: git2::Repository, project: projects::Project) -> Self {
        Self {
            project,
            git_repository,
        }
    }

    pub fn read<P: AsRef<Path>>(&self, file_path: P) -> Result<Option<Vec<deltas::Delta>>> {
        let file_deltas_path = self.project.deltas_path().join(file_path);
        if !file_deltas_path.exists() {
            return Ok(None);
        }

        let file_deltas = std::fs::read_to_string(&file_deltas_path).with_context(|| {
            format!(
                "failed to read file deltas from {}",
                file_deltas_path.to_str().unwrap()
            )
        })?;

        let deltas: Vec<deltas::Delta> = serde_json::from_str(&file_deltas).with_context(|| {
            format!(
                "failed to parse file deltas from {}",
                file_deltas_path.to_str().unwrap()
            )
        })?;

        Ok(Some(deltas))
    }

    pub fn write<P: AsRef<Path>>(
        &self,
        file_path: P,
        deltas: &Vec<deltas::Delta>,
    ) -> Result<sessions::Session> {
        // make sure we always have a session before writing deltas
        let session = match sessions::Session::current(&self.git_repository, &self.project)? {
            Some(mut session) => {
                session
                    .touch(&self.project)
                    .with_context(|| format!("failed to touch session {}", session.id))?;
                Ok(session)
            }
            None => sessions::Session::from_head(&self.git_repository, &self.project),
        }?;

        let delta_path = self.project.deltas_path().join(file_path);
        let delta_dir = delta_path.parent().unwrap();
        std::fs::create_dir_all(&delta_dir)?;
        log::info!(
            "{}: writing deltas to {}",
            &self.project.id,
            delta_path.to_str().unwrap()
        );
        let raw_deltas = serde_json::to_string(&deltas)?;
        std::fs::write(delta_path.clone(), raw_deltas).with_context(|| {
            format!(
                "failed to write file deltas to {}",
                delta_path.to_str().unwrap()
            )
        })?;

        Ok(session)
    }

    // returns deltas for a current session from .gb/session/deltas tree
    fn list_current_deltas(
        &self,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let deltas_path = self.project.deltas_path();
        if !deltas_path.exists() {
            return Ok(HashMap::new());
        }

        let file_paths = fs::list_files(&deltas_path).with_context(|| {
            format!("Failed to list files in {}", deltas_path.to_str().unwrap())
        })?;

        let deltas = file_paths
            .iter()
            .map_while(|file_path| {
                if let Some(paths) = &paths {
                    if !paths.contains(&file_path.to_str().unwrap()) {
                        return None;
                    }
                }
                let file_deltas = self.read(Path::new(file_path));
                match file_deltas {
                    Ok(Some(file_deltas)) => {
                        Some(Ok((file_path.to_str().unwrap().to_string(), file_deltas)))
                    }
                    Ok(None) => None,
                    Err(err) => Some(Err(err)),
                }
            })
            .collect::<Result<HashMap<String, Vec<deltas::Delta>>>>()?;

        Ok(deltas)
    }

    pub fn list(
        &self,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let reference = self
            .git_repository
            .find_reference(&self.project.refname())?;
        let session =
            match sessions::get(&self.git_repository, &self.project, &reference, session_id)? {
                Some(session) => Ok(session),
                None => Err(anyhow!("Session {} not found", session_id)),
            }?;

        if session.hash.is_none() {
            self.list_current_deltas(paths).with_context(|| {
                format!("Failed to list current deltas for session {}", session_id)
            })
        } else {
            self.list_commit_deltas(&session.hash.unwrap(), paths)
                .with_context(|| format!("Failed to list commit deltas for session {}", session_id))
        }
    }

    // returns deltas from gitbutler commit's session/deltas tree
    fn list_commit_deltas(
        &self,
        commit_hash: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
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
