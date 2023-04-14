use anyhow::Result;
use tempfile::tempdir;

use crate::{app::gb_repository, projects, storage};

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

fn project_store(project: &projects::Project) -> Result<projects::Storage> {
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let store = projects::Storage::new(storage);
    store.add_project(project)?;
    Ok(store)
}

#[test]
fn test_get_current_session_writer_should_use_existing_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let project_store = project_store(&project)?;
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id.clone(), project_store.clone())?;

    let current_session_1 = gb_repo.get_or_create_current_session()?;
    let current_session_2 = gb_repo.get_or_create_current_session()?;
    assert_eq!(current_session_1.id, current_session_2.id);

    Ok(())
}
