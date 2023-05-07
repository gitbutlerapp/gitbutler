use anyhow::Result;
use tempfile::tempdir;

use crate::{
    app::{deltas, gb_repository},
    projects, storage, users,
};

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

fn commit_all(repository: &git2::Repository) -> Result<git2::Oid> {
    let mut index = repository.index()?;
    index.add_all(&["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index.write_tree()?;
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let commit_oid = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "some commit",
        &repository.find_tree(oid)?,
        &[&repository.find_commit(repository.refname_to_id("HEAD")?)?],
    )?;
    Ok(commit_oid)
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
fn test_must_not_return_init_session() -> Result<()> {
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

    assert!(gb_repo.get_current_session()?.is_none());

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

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
    assert!(session.is_none());

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

    Ok(())
}

#[test]
fn test_init_on_non_empty_repository() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);

    std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
    commit_all(&repository)?;

    gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    Ok(())
}

#[test]
fn test_flush_on_existing_repository() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);

    std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
    commit_all(&repository)?;

    gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    gb_repo.get_or_create_current_session()?;
    gb_repo.flush()?;

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
    assert!(session.is_some());
    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 1);

    Ok(())
}

#[test]
fn test_list_deltas_from_current_session() -> Result<()> {
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

    let current_session = gb_repo.get_or_create_current_session()?;
    let writer = gb_repo.get_session_writer(&current_session)?;
    writer.write_deltas(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;

    let reader = gb_repo.get_session_reader(&current_session)?;
    let deltas = reader.deltas(None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas.get("test.txt").unwrap()[0].operations.len(), 1);
    assert_eq!(
        deltas.get("test.txt").unwrap()[0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_deltas_from_flushed_session() -> Result<()> {
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

    let current_session = gb_repo.get_or_create_current_session()?;
    let writer = gb_repo.get_session_writer(&current_session)?;
    writer.write_deltas(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session = gb_repo.flush()?;

    let reader = gb_repo.get_session_reader(&session.unwrap())?;
    let deltas = reader.deltas(None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas.get("test.txt").unwrap()[0].operations.len(), 1);
    assert_eq!(
        deltas.get("test.txt").unwrap()[0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_files_from_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);

    // files are there before the session is created
    std::fs::write(
        repository.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let session = gb_repo.get_or_create_current_session()?;

    let reader = gb_repo.get_session_reader(&session)?;
    let files = reader.files(None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(files.get("test.txt").unwrap(), "Hello World");

    Ok(())
}

#[test]
fn test_list_files_from_flushed_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);

    // files are there before the session is created
    std::fs::write(
        repository.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    gb_repo.get_or_create_current_session()?;
    let session = gb_repo.flush()?.unwrap();

    let reader = gb_repo.get_session_reader(&session)?;
    let files = reader.files(None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(files.get("test.txt").unwrap(), "Hello World");

    Ok(())
}
