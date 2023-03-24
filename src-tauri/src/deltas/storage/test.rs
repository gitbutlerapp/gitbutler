use crate::{deltas::Delta, deltas::Operation, projects, sessions};
use anyhow::Result;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tempfile::tempdir;

fn test_project() -> Result<(git2::Repository, projects::Project)> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(&path)?;
    let repo = git2::Repository::init(&path)?;
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let sig = git2::Signature::now("test", "test@email.com").unwrap();
    let _commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "initial commit",
        &repo.find_tree(oid)?,
        &[],
    )?;
    let project = projects::Project::from_path(path)?;
    Ok((repo, project))
}

#[test]
fn test_read_none() {
    let (repo, project) = test_project().unwrap();
    let repo = Arc::new(Mutex::new(repo));
    let sessions_store = sessions::Store::new(repo.clone(), project.clone());
    let store = super::Store::new(repo, project, sessions_store);
    let file_path = Path::new("test.txt");
    let deltas = store.read(file_path);
    assert!(deltas.is_ok());
    assert!(deltas.unwrap().is_none());
}

#[test]
fn test_read_invalid() {
    let (repo, project) = test_project().unwrap();
    let repo = Arc::new(Mutex::new(repo));
    let sessions_store = sessions::Store::new(repo.clone(), project.clone());
    let store = super::Store::new(repo, project.clone(), sessions_store);
    let file_path = Path::new("test.txt");
    let full_file_path = project.deltas_path().join(file_path);

    std::fs::create_dir_all(full_file_path.parent().unwrap()).unwrap();
    std::fs::write(full_file_path, "invalid").unwrap();

    let deltas = store.read(file_path);
    assert!(deltas.is_err());
}

#[test]
fn test_write_read() {
    let (repo, project) = test_project().unwrap();
    let repo = Arc::new(Mutex::new(repo));
    let sessions_store = sessions::Store::new(repo.clone(), project.clone());
    let store = super::Store::new(repo, project, sessions_store);
    let file_path = Path::new("test.txt");

    let deltas = vec![Delta {
        operations: vec![Operation::Insert((0, "Hello, world!".to_string()))],
        timestamp_ms: 0,
    }];
    let write_result = store.write(file_path, &deltas);
    assert!(write_result.is_ok());

    let read_result = store.read(file_path);
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap().unwrap(), deltas);
}

#[test]
fn test_write_must_create_session() {
    let (repo, project) = test_project().unwrap();
    let repo = Arc::new(Mutex::new(repo));
    let sessions_store = sessions::Store::new(repo.clone(), project.clone());
    let store = super::Store::new(repo, project, sessions_store.clone());
    let file_path = Path::new("test.txt");

    let deltas = vec![Delta {
        operations: vec![Operation::Insert((0, "Hello, world!".to_string()))],
        timestamp_ms: 0,
    }];
    let write_result = store.write(file_path, &deltas);
    assert!(write_result.is_ok());

    let current_session = sessions_store.get_current();
    assert!(current_session.is_ok());
    let current_session = current_session.unwrap();

    assert!(current_session.is_some());
}

#[test]
fn test_write_must_not_override_session() {
    let (repo, project) = test_project().unwrap();
    let repo = Arc::new(Mutex::new(repo));
    let sessions_store = sessions::Store::new(repo.clone(), project.clone());
    let store = super::Store::new(repo, project, sessions_store.clone());
    let file_path = Path::new("test.txt");

    let session_before_write = sessions_store.create_current();
    assert!(session_before_write.is_ok());
    let session_before_write = session_before_write.unwrap();

    let deltas = vec![Delta {
        operations: vec![Operation::Insert((0, "Hello, world!".to_string()))],
        timestamp_ms: 0,
    }];
    let write_result = store.write(file_path, &deltas);
    assert!(write_result.is_ok());

    let session_after_write = sessions_store.get_current();
    assert!(session_after_write.is_ok());
    let session_after_write = session_after_write.unwrap();

    assert!(session_after_write.is_some());
    let session_after_write = session_after_write.unwrap();

    assert_eq!(session_before_write.id, session_after_write.id);
}
