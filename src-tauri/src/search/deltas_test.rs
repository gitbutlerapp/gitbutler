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
fn test_simple() {
    let (repo, project) = test_project().unwrap();
    let index_path = tempdir().unwrap().path().to_str().unwrap().to_string();

    let mut session = sessions::Session::from_head(&repo, &project).unwrap();
    deltas::write(
        &repo,
        &project,
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![Operation::Insert((5, " World".to_string()))],
                timestamp_ms: 0,
            },
        ],
    )
    .unwrap();
    session.flush(&repo, &None, &project).unwrap();

    let index = super::DeltasIndex::open_or_create(&index_path, &project).unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let write_result = index.write(&session, &repo, &project, &reference);
    assert!(write_result.is_ok());

    let session_hash = session.hash.unwrap();

    let search_result1 = index.search("hello");
    assert!(search_result1.is_ok());
    let search_result1 = search_result1.unwrap();
    assert_eq!(search_result1.len(), 1);
    assert_eq!(search_result1[0].session_hash, session_hash);
    assert_eq!(search_result1[0].file_path, "test.txt");
    assert_eq!(search_result1[0].index, 0);

    let search_result2 = index.search("world");
    assert!(search_result2.is_ok());
    let search_result2 = search_result2.unwrap();
    assert_eq!(search_result2.len(), 1);
    assert_eq!(search_result2[0].session_hash, session_hash);
    assert_eq!(search_result2[0].file_path, "test.txt");
    assert_eq!(search_result2[0].index, 1);

    let search_result3 = index.search("hello world");
    assert!(search_result3.is_ok());
    let search_result3 = search_result3.unwrap();
    assert_eq!(search_result3.len(), 2);
    assert_eq!(search_result3[0].session_hash, session_hash);
    assert_eq!(search_result3[0].file_path, "test.txt");
    assert_eq!(search_result3[1].session_hash, session_hash);
    assert_eq!(search_result3[1].file_path, "test.txt");

    let search_by_filename_result = index.search("test.txt");
    assert!(search_by_filename_result.is_ok());
    let search_by_filename_result = search_by_filename_result.unwrap();
    assert_eq!(search_by_filename_result.len(), 2);
    assert_eq!(search_by_filename_result[0].session_hash, session_hash);
    assert_eq!(search_by_filename_result[0].file_path, "test.txt");
}
