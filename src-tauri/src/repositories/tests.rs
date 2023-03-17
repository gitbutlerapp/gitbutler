use crate::{projects, repositories::Store, storage, users};
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
fn test_open_creates_reference() {
    let storage_path = tempdir().unwrap();
    let storage = storage::Storage::from_path(storage_path.path().to_path_buf());

    let project = test_project().unwrap();
    let projects_storage = projects::Storage::new(storage.clone());
    projects_storage.add_project(&project).unwrap();

    let users_storage = users::Storage::new(storage.clone());
    let mut store = Store::new(projects_storage.clone(), users_storage.clone());

    let repository = store.get(&project.id);
    assert!(repository.is_ok());
    let repository = repository.unwrap();

    assert!(repository
        .git_repository
        .find_reference(&project.refname())
        .is_ok());
}
