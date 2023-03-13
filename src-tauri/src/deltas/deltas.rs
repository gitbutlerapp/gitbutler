use super::operations;
use crate::{fs, projects, sessions};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub operations: Vec<operations::Operation>,
    pub timestamp_ms: u128,
}

pub fn read(project: &projects::Project, file_path: &Path) -> Result<Option<Vec<Delta>>> {
    let file_deltas_path = project.deltas_path().join(file_path);
    if !file_deltas_path.exists() {
        return Ok(None);
    }

    let file_deltas = std::fs::read_to_string(&file_deltas_path).with_context(|| {
        format!(
            "failed to read file deltas from {}",
            file_deltas_path.to_str().unwrap()
        )
    })?;

    let deltas: Vec<Delta> = serde_json::from_str(&file_deltas).with_context(|| {
        format!(
            "failed to parse file deltas from {}",
            file_deltas_path.to_str().unwrap()
        )
    })?;

    Ok(Some(deltas))
}

pub fn write(
    repo: &git2::Repository,
    project: &projects::Project,
    file_path: &Path,
    deltas: &Vec<Delta>,
) -> Result<sessions::Session> {
    // make sure we always have a session before writing deltas
    let session = match sessions::Session::current(repo, project)? {
        Some(mut session) => {
            session
                .touch(project)
                .with_context(|| format!("failed to touch session {}", session.id))?;
            Ok(session)
        }
        None => sessions::Session::from_head(repo, project),
    }?;

    let delta_path = project.deltas_path().join(file_path);
    let delta_dir = delta_path.parent().unwrap();
    std::fs::create_dir_all(&delta_dir)?;
    log::info!(
        "{}: writing deltas to {}",
        project.id,
        delta_path.to_str().unwrap()
    );
    let raw_deltas = serde_json::to_string(&deltas)?;
    log::debug!("{}: raw deltas: {}", project.id, raw_deltas);
    std::fs::write(delta_path.clone(), raw_deltas).with_context(|| {
        format!(
            "failed to write file deltas to {}",
            delta_path.to_str().unwrap()
        )
    })?;

    Ok(session)
}

// returns deltas for a current session from .gb/session/deltas tree
fn list_current_deltas(project: &projects::Project) -> Result<HashMap<String, Vec<Delta>>> {
    let deltas_path = project.deltas_path();
    if !deltas_path.exists() {
        return Ok(HashMap::new());
    }

    let file_paths = fs::list_files(&deltas_path)
        .with_context(|| format!("Failed to list files in {}", deltas_path.to_str().unwrap()))?;

    let deltas = file_paths
        .iter()
        .map_while(|file_path| {
            let file_deltas = read(project, Path::new(file_path));
            match file_deltas {
                Ok(Some(file_deltas)) => Some(Ok((file_path.to_owned(), file_deltas))),
                Ok(None) => None,
                Err(err) => Some(Err(err)),
            }
        })
        .collect::<Result<HashMap<String, Vec<Delta>>>>()?;

    Ok(deltas)
}

pub fn list(
    repo: &git2::Repository,
    project: &projects::Project,
    reference: &git2::Reference,
    session_id: &str,
) -> Result<HashMap<String, Vec<Delta>>> {
    let session = match sessions::get(repo, project, reference, session_id)? {
        Some(session) => Ok(session),
        None => Err(anyhow!("Session {} not found", session_id)),
    }?;

    if session.hash.is_none() {
        list_current_deltas(project)
            .with_context(|| format!("Failed to list current deltas for session {}", session_id))
    } else {
        list_commit_deltas(repo, &session.hash.unwrap())
            .with_context(|| format!("Failed to list commit deltas for session {}", session_id))
    }
}

// returns deltas from gitbutler commit's session/deltas tree
fn list_commit_deltas(
    repo: &git2::Repository,
    commit_hash: &str,
) -> Result<HashMap<String, Vec<Delta>>> {
    let commit_id = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(commit_id)?;
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

        let blob = entry.to_object(repo).and_then(|obj| obj.peel_to_blob());
        let content = blob.map(|blob| blob.content().to_vec());

        match content {
            Ok(content) => {
                let deltas: Result<Vec<Delta>> =
                    serde_json::from_slice(&content).map_err(|e| e.into());
                blobs.insert(
                    entry_path
                        .strip_prefix("session/deltas")
                        .unwrap()
                        .to_owned(),
                    deltas,
                );
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
