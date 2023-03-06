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
fn test_sorted_by_timestamp() {
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
                timestamp_ms: 1,
            },
        ],
    )
    .unwrap();
    session.flush(&repo, &None, &project).unwrap();

    let mut searcher = super::Deltas::at(index_path.into()).unwrap();

    let write_result = searcher.index_session(&repo, &project, &session);
    assert!(write_result.is_ok());

    let search_result = searcher.search(&super::SearchQuery {
        project_id: project.id,
        q: "hello world".to_string(),
        limit: 10,
        ..Default::default()
    });
    assert!(search_result.is_ok());
    let search_result = search_result.unwrap();
    assert_eq!(search_result.len(), 2);
    assert_eq!(search_result[0].index, 1);
    assert_eq!(search_result[1].index, 0);
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

    let mut searcher = super::Deltas::at(index_path.into()).unwrap();

    let write_result = searcher.index_session(&repo, &project, &session);
    assert!(write_result.is_ok());

    let search_result1 = searcher.search(&super::SearchQuery {
        project_id: project.id.clone(),
        q: "hello".to_string(),
        limit: 10,
        ..Default::default()
    });
    assert!(search_result1.is_ok());
    let search_result1 = search_result1.unwrap();
    assert_eq!(search_result1.len(), 1);
    assert_eq!(search_result1[0].session_id, session.id);
    assert_eq!(search_result1[0].project_id, project.id);
    assert_eq!(search_result1[0].file_path, "test.txt");
    assert_eq!(search_result1[0].index, 0);

    let search_result2 = searcher.search(&super::SearchQuery {
        project_id: project.id.clone(),
        q: "world".to_string(),
        limit: 10,
        ..Default::default()
    });
    assert!(search_result2.is_ok());
    let search_result2 = search_result2.unwrap();
    assert_eq!(search_result2.len(), 1);
    assert_eq!(search_result2[0].session_id, session.id);
    assert_eq!(search_result2[0].project_id, project.id);
    assert_eq!(search_result2[0].file_path, "test.txt");
    assert_eq!(search_result2[0].index, 1);

    let search_result3 = searcher.search(&super::SearchQuery {
        project_id: project.id.clone(),
        q: "hello world".to_string(),
        limit: 10,
        ..Default::default()
    });
    println!("{:?}", search_result3);
    assert!(search_result3.is_ok());
    let search_result3 = search_result3.unwrap();
    assert_eq!(search_result3.len(), 2);
    assert_eq!(search_result3[0].project_id, project.id);
    assert_eq!(search_result3[0].session_id, session.id);
    assert_eq!(search_result3[0].file_path, "test.txt");
    assert_eq!(search_result3[1].session_id, session.id);
    assert_eq!(search_result3[1].project_id, project.id);
    assert_eq!(search_result3[1].file_path, "test.txt");

    let search_by_filename_result = searcher.search(&super::SearchQuery {
        project_id: project.id.clone(),
        q: "test.txt".to_string(),
        limit: 10,
        ..Default::default()
    });
    assert!(search_by_filename_result.is_ok());
    let search_by_filename_result = search_by_filename_result.unwrap();
    assert_eq!(search_by_filename_result.len(), 2);
    assert_eq!(search_by_filename_result[0].session_id, session.id);
    assert_eq!(search_by_filename_result[0].project_id, project.id);
    assert_eq!(search_by_filename_result[0].file_path, "test.txt");

    let not_found_result = searcher.search(&super::SearchQuery {
        project_id: "not found".to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        ..Default::default()
    });
    assert!(not_found_result.is_ok());
    let not_found_result = not_found_result.unwrap();
    assert_eq!(not_found_result.len(), 0);
}
