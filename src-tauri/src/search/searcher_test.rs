use std::{path::Path, time};

use anyhow::Result;
use tempfile::tempdir;

use crate::{bookmarks, deltas, gb_repository, projects, storage, users};

fn test_repository() -> Result<git2::Repository> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    let repository = git2::Repository::init(path)?;
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
fn test_sorted_by_timestamp() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " Hello".to_string()))],
                timestamp_ms: 1,
            },
        ],
    )?;
    let session = gb_repo.flush()?;

    let searcher = super::Searcher::at(index_path).unwrap();

    let write_result = searcher.index_session(&gb_repo, &session.unwrap());
    assert!(write_result.is_ok());

    let search_result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello".to_string(),
        limit: 10,
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
fn search_by_bookmark_note() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
            timestamp_ms: 123456,
        }],
    )?;
    let session = gb_repo.flush()?.unwrap();

    let searcher = super::Searcher::at(index_path).unwrap();

    // first we index bookmark
    searcher.index_bookmark(&bookmarks::Bookmark {
        project_id: gb_repo.get_project_id().to_string(),
        timestamp_ms: 123456,
        created_timestamp_ms: 0,
        updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
        note: "bookmark note".to_string(),
        deleted: false,
    })?;
    // and should not be able to find it before delta on the same timestamp is indexed
    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "bookmark".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 0);

    // then index session with deltas
    searcher.index_session(&gb_repo, &session)?;

    // delta should be found by diff
    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);

    // and by note
    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "bookmark".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);

    // then update the note
    searcher.index_bookmark(&bookmarks::Bookmark {
        project_id: gb_repo.get_project_id().to_string(),
        timestamp_ms: 123456,
        created_timestamp_ms: 0,
        updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
        note: "updated bookmark note".to_string(),
        deleted: false,
    })?;

    // should be able to find it by diff still
    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);

    // and by new note
    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "updated bookmark".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[test]
fn search_by_full_match() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session = gb_repo.flush()?;
    let session = session.unwrap();

    let searcher = super::Searcher::at(index_path).unwrap();

    let write_result = searcher.index_session(&gb_repo, &session);
    assert!(write_result.is_ok());

    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "hello world".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 0);

    Ok(())
}

#[test]
fn search_by_diff() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " World".to_string()))],
                timestamp_ms: 0,
            },
        ],
    )?;
    let session = gb_repo.flush()?;
    let session = session.unwrap();

    let searcher = super::Searcher::at(index_path).unwrap();

    let write_result = searcher.index_session(&gb_repo, &session);
    assert!(write_result.is_ok());

    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "world".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);
    assert_eq!(result.page[0].session_id, session.id);
    assert_eq!(result.page[0].project_id, gb_repo.get_project_id());
    assert_eq!(result.page[0].file_path, "test.txt");
    assert_eq!(result.page[0].index, 1);

    Ok(())
}

#[test]
fn should_index_bookmark_once() -> Result<()> {
    let index_path = tempdir()?.path().to_str().unwrap().to_string();
    let searcher = super::Searcher::at(index_path).unwrap();

    // should not index deleted non-existing bookmark
    assert!(searcher
        .index_bookmark(&bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            created_timestamp_ms: 0,
            updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
            note: "bookmark text note".to_string(),
            deleted: true,
        })?
        .is_none());

    // should index new non deleted bookmark
    assert!(searcher
        .index_bookmark(&bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            created_timestamp_ms: 0,
            updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
            note: "bookmark text note".to_string(),
            deleted: false,
        })?
        .is_some());

    // should not index existing non deleted bookmark
    assert!(searcher
        .index_bookmark(&bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            created_timestamp_ms: 0,
            updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
            note: "bookmark text note".to_string(),
            deleted: false,
        })?
        .is_none());

    // should index existing deleted bookmark
    assert!(searcher
        .index_bookmark(&bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            created_timestamp_ms: 0,
            updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
            note: "bookmark text note".to_string(),
            deleted: true,
        })?
        .is_some());

    // should not index existing deleted bookmark
    assert!(searcher
        .index_bookmark(&bookmarks::Bookmark {
            project_id: "test".to_string(),
            timestamp_ms: 0,
            created_timestamp_ms: 0,
            updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
            note: "bookmark text note".to_string(),
            deleted: true,
        })?
        .is_none());

    Ok(())
}

#[test]
fn test_delete_all() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, "World".to_string()))],
                timestamp_ms: 1,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " ".to_string()))],
                timestamp_ms: 2,
            },
        ],
    )?;
    let session = gb_repo.flush()?;
    let searcher = super::Searcher::at(index_path)?;
    searcher.index_session(&gb_repo, &session.unwrap())?;

    searcher.delete_all_data()?;

    let search_result_from = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(search_result_from.total, 0);

    Ok(())
}

#[test]
fn search_bookmark_by_phrase() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session = gb_repo.flush()?;
    let session = session.unwrap();

    let searcher = super::Searcher::at(index_path).unwrap();

    searcher.index_session(&gb_repo, &session)?;
    searcher.index_bookmark(&bookmarks::Bookmark {
        project_id: gb_repo.get_project_id().to_string(),
        timestamp_ms: 0,
        created_timestamp_ms: 0,
        updated_timestamp_ms: time::UNIX_EPOCH.elapsed()?.as_millis(),
        note: "bookmark text note".to_string(),
        deleted: false,
    })?;

    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "bookmark note".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 0);

    let result = searcher.search(&super::Query {
        project_id: gb_repo.get_project_id().to_string(),
        q: "text note".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(result.total, 1);

    Ok(())
}

#[test]
fn search_by_filename() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let project_store = projects::Storage::new(storage.clone());
    project_store.add_project(&project)?;
    let user_store = users::Storage::new(storage);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

    let index_path = tempdir()?.path().to_str().unwrap().to_string();

    let writer = deltas::Writer::new(&gb_repo)?;
    writer.write(
        Path::new("test.txt"),
        &vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "Hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, "World".to_string()))],
                timestamp_ms: 1,
            },
        ],
    )?;
    let session = gb_repo.flush()?;
    let session = session.unwrap();

    let searcher = super::Searcher::at(index_path).unwrap();

    searcher.index_session(&gb_repo, &session)?;

    let found_result = searcher
        .search(&super::Query {
            project_id: gb_repo.get_project_id().to_string(),
            q: "test.txt".to_string(),
            limit: 10,
            offset: None,
        })?
        .page;
    assert_eq!(found_result.len(), 2);
    assert_eq!(found_result[0].session_id, session.id);
    assert_eq!(found_result[0].project_id, gb_repo.get_project_id());
    assert_eq!(found_result[0].file_path, "test.txt");

    let not_found_result = searcher.search(&super::Query {
        project_id: "not found".to_string(),
        q: "test.txt".to_string(),
        limit: 10,
        offset: None,
    })?;
    assert_eq!(not_found_result.total, 0);

    Ok(())
}
