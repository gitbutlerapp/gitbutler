use std::path::Path;

use crate::{
    deltas::{self, Operation},
    projects, sessions,
};
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

#[test]
fn test_index_session() {
    let (repo, project) = test_project().unwrap();
    let index_path = tempdir().unwrap().path().to_str().unwrap().to_string();
    println!("index_path: {}", index_path);

    let mut session = sessions::Session::from_head(&repo, &project).unwrap();
    deltas::write(
        &repo,
        &project,
        Path::new("test.txt"),
        &vec![deltas::Delta {
            operations: vec![Operation::Insert((0, "Hello, world!".to_string()))],
            timestamp_ms: 0,
        }],
    )
    .unwrap();
    session.flush(&repo, &None, &project).unwrap();

    let index = super::DeltasIndex::open_or_create(&index_path, &project).unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let write_result = index.write(&session, &repo, &project, &reference);
    assert!(write_result.is_ok());
}
