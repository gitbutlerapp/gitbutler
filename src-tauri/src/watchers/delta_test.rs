use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{deltas, projects, sessions};
use anyhow::Result;
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

fn clone_repo(repo: &git2::Repository) -> git2::Repository {
    git2::Repository::open(repo.path()).unwrap()
}

#[test]
fn test_register_file_change_must_create_session() {
    let (repo, project) = test_project().unwrap();
    let arepo = Arc::new(Mutex::new(clone_repo(&repo)));

    let relative_file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(relative_file_path), "test").unwrap();

    let sessions_storage = sessions::Store::new(arepo.clone(), project.clone());
    let deltas_storage = deltas::Store::new(arepo, project.clone(), sessions_storage);
    let result =
        super::delta::register_file_change(&project, &repo, &deltas_storage, &relative_file_path);
    println!("{:?}", result);
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (session, deltas) = maybe_session_deltas.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(session.hash, None);
}

#[test]
fn test_register_file_change_must_not_change_session() {
    let (repo, project) = test_project().unwrap();
    let arepo = Arc::new(Mutex::new(clone_repo(&repo)));

    let relative_file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(relative_file_path), "test").unwrap();

    let sessions_storage = sessions::Store::new(arepo.clone(), project.clone());
    let deltas_storage = deltas::Store::new(arepo, project.clone(), sessions_storage);
    let result =
        super::delta::register_file_change(&project, &repo, &deltas_storage, &relative_file_path);
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (session1, deltas1) = maybe_session_deltas.unwrap();
    assert_eq!(deltas1.len(), 1);

    std::fs::write(Path::new(&project.path).join(relative_file_path), "test2").unwrap();

    let result =
        super::delta::register_file_change(&project, &repo, &deltas_storage, &relative_file_path);
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (session2, deltas2) = maybe_session_deltas.unwrap();
    assert_eq!(deltas2.len(), 2);
    assert_eq!(deltas2[0], deltas1[0]);
    assert_eq!(session1.id, session2.id);
}
