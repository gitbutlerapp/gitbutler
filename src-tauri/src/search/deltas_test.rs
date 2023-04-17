use core::ops::Range;
use std::path::Path;

use anyhow::Result;
use tempfile::tempdir;

use crate::{
    app,
    deltas::{self, Operation},
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

#[test]
fn test_filter_by_timestamp() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = app::gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let session = gb_repo.get_or_create_current_session()?;
    let writer = gb_repo.get_session_writer(&session)?;
    writer.write_deltas(
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![Operation::Insert((5, "World".to_string()))],
                timestamp_ms: 1,
            },
            deltas::Delta {
                operations: vec![Operation::Insert((5, " ".to_string()))],
                timestamp_ms: 2,
            },
        ],
    )?;
    let session = gb_repo.flush()?;

    let searcher = super::Deltas::at(index_path)?;

    searcher.index_session(&gb_repo, &session.unwrap())?;

    let search_result_from = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        range: Range { start: 2, end: 10 },
        offset: None,
    })?;
    assert_eq!(search_result_from.total, 1);
    assert_eq!(search_result_from.page[0].index, 2);

    let search_result_to = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        range: Range { start: 0, end: 1 },
        offset: None,
    })?;
    assert_eq!(search_result_to.total, 1);
    assert_eq!(search_result_to.page[0].index, 0);

    let search_result_from_to = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        range: Range { start: 1, end: 2 },
        offset: None,
    })?;
    assert_eq!(search_result_from_to.total, 1);
    assert_eq!(search_result_from_to.page[0].index, 1);

    Ok(())
}

#[test]
fn test_sorted_by_timestamp() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = app::gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let session = gb_repo.get_or_create_current_session()?;
    let writer = gb_repo.get_session_writer(&session)?;
    writer.write_deltas(
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
    )?;
    let session = gb_repo.flush()?;

    let searcher = super::Deltas::at(index_path).unwrap();

    let write_result = searcher.index_session(&gb_repo, &session.unwrap());
    assert!(write_result.is_ok());

    let search_result = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello world".to_string(),
        limit: 10,
        range: Range { start: 0, end: 10 },
        offset: None,
    });
    assert!(search_result.is_ok());
    let search_result = search_result.unwrap();
    assert_eq!(search_result.total, 2);
    assert_eq!(search_result.page[0].index, 1);
    assert_eq!(search_result.page[1].index, 0);

    Ok(())
}

#[test]
fn test_simple() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo = app::gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let session = gb_repo.get_or_create_current_session()?;
    let writer = gb_repo.get_session_writer(&session)?;

    writer.write_deltas(
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
    )?;
    let session = gb_repo.flush()?;
    let session = session.unwrap();

    let searcher = super::Deltas::at(index_path).unwrap();

    let write_result = searcher.index_session(&gb_repo, &session);
    assert!(write_result.is_ok());

    let search_result1 = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello".to_string(),
        limit: 10,
        offset: None,
        range: Range { start: 0, end: 10 },
    });
    println!("{:?}", search_result1);
    assert!(search_result1.is_ok());
    let search_result1 = search_result1.unwrap();
    assert_eq!(search_result1.total, 1);
    assert_eq!(search_result1.page[0].session_id, session.id);
    assert_eq!(search_result1.page[0].project_id, gb_repo.get_project_id());
    assert_eq!(search_result1.page[0].file_path, "test.txt");
    assert_eq!(search_result1.page[0].index, 0);

    let search_result2 = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "world".to_string(),
        limit: 10,
        offset: None,
        range: Range { start: 0, end: 10 },
    });
    assert!(search_result2.is_ok());
    let search_result2 = search_result2.unwrap().page;
    assert_eq!(search_result2.len(), 1);
    assert_eq!(search_result2[0].session_id, session.id);
    assert_eq!(search_result2[0].project_id, gb_repo.get_project_id());
    assert_eq!(search_result2[0].file_path, "test.txt");
    assert_eq!(search_result2[0].index, 1);

    let search_result3 = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello world".to_string(),
        limit: 10,
        offset: None,
        range: Range { start: 0, end: 10 },
    });
    assert!(search_result3.is_ok());
    let search_result3 = search_result3.unwrap().page;
    assert_eq!(search_result3.len(), 2);
    assert_eq!(search_result3[0].project_id, gb_repo.get_project_id());
    assert_eq!(search_result3[0].session_id, session.id);
    assert_eq!(search_result3[0].file_path, "test.txt");
    assert_eq!(search_result3[1].session_id, session.id);
    assert_eq!(search_result3[1].project_id, gb_repo.get_project_id());
    assert_eq!(search_result3[1].file_path, "test.txt");

    let search_by_filename_result = searcher.search(&super::SearchQuery {
        project_id: gb_repo.get_project_id().to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        offset: None,
        range: Range { start: 0, end: 10 },
    });
    assert!(search_by_filename_result.is_ok());
    let search_by_filename_result = search_by_filename_result.unwrap().page;
    assert_eq!(search_by_filename_result.len(), 2);
    assert_eq!(search_by_filename_result[0].session_id, session.id);
    assert_eq!(
        search_by_filename_result[0].project_id,
        gb_repo.get_project_id()
    );
    assert_eq!(search_by_filename_result[0].file_path, "test.txt");

    let not_found_result = searcher.search(&super::SearchQuery {
        project_id: "not found".to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        offset: None,
        range: Range { start: 0, end: 10 },
    });
    assert!(not_found_result.is_ok());
    let not_found_result = not_found_result.unwrap();
    assert_eq!(not_found_result.total, 0);

    Ok(())
}
