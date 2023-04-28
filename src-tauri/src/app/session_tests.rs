use anyhow::Result;
use tempfile::tempdir;

use crate::{app::gb_repository, projects, sessions, storage, users};

use super::session::SessionWriter;

fn test_repository() -> Result<git2::Repository> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    let repository = git2::Repository::init(&path)?;
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

fn test_project(repository: &git2::Repository) -> Result<projects::Project> {
    let project = projects::Project::from_path(
        repository
            .path()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
    )?;
    Ok(project)
}

#[test]
fn test_should_not_write_session_with_hash() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: Some("hash".to_string()),
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
        activity: vec![],
    };

    assert!(SessionWriter::open(&gb_repo, &session).is_err());

    Ok(())
}

#[test]
fn test_should_write_full_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
        activity: vec![],
    };

    SessionWriter::open(&gb_repo, &session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/id"))?,
        "session_id"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/commit"))?,
        "commit"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/branch"))?,
        "branch"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/last"))?,
        "1" 
    );

    Ok(())
}

#[test]
fn test_should_write_partial_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: None,
            commit: None,
        },
        activity: vec![],
    };

    SessionWriter::open(&gb_repo, &session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/id"))?,
        "session_id"
    );
    assert!(!gb_repo.session_path().join("meta/commit").exists());
    assert!(!gb_repo.session_path().join("meta/branch").exists());
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/last"))?,
        "1"
    );

    Ok(())
}
