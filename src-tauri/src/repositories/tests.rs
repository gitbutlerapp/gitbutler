use super::Repository;
use crate::{projects, storage, users};
use anyhow::Result;
use tempfile::tempdir;

fn test_project() -> Result<projects::Project> {
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
    Ok(project)
}

#[test]
fn test_open_always_with_session() {
    let storage_path = tempdir().unwrap();
    let storage = storage::Storage::from_path(storage_path.path().to_path_buf());

    let project = test_project().unwrap();
    let projects_storage = projects::Storage::new(storage.clone());
    projects_storage.add_project(&project).unwrap();

    let users_storage = users::Storage::new(storage.clone());

    let repository = Repository::open(&projects_storage, &users_storage, &project.id);
    assert!(repository.is_ok());
    let repository = repository.unwrap();

    let sessions = repository.sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].hash.is_some());
}
