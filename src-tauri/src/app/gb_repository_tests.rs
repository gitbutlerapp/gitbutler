use anyhow::Result;
use tempfile::tempdir;

use crate::{app::gb_repository, projects, storage, users};

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
fn test_get_current_session_writer_should_use_existing_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let current_session_1 = gb_repo.get_or_create_current_session()?;
    let current_session_2 = gb_repo.get_or_create_current_session()?;
    assert_eq!(current_session_1.id, current_session_2.id);

    Ok(())
}

#[test]
fn test_must_flush_on_init() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 1);
    assert!(gb_repo.get_current_session()?.is_none());

    Ok(())
}

#[test]
fn test_must_not_flush_without_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let session = gb_repo.flush()?;

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 1);
    assert!(session.is_none());

    Ok(())
}

#[test]
fn test_must_flush_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    gb_repo.get_or_create_current_session()?;

    let session = gb_repo.flush()?;

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 2);
    assert!(session.is_some());

    Ok(())
}
