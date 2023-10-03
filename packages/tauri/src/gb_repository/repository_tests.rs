use std::{path, thread, time};

use crate::{deltas, gb_repository, projects, reader, sessions, test_utils, users};
use anyhow::Result;
use tempfile::tempdir;

fn remote_repository() -> Result<git2::Repository> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    let repository = git2::Repository::init_bare(path)?;
    Ok(repository)
}

#[test]
fn test_get_current_session_writer_should_use_existing_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let current_session_1 = gb_repo.get_or_create_current_session()?;
    let current_session_2 = gb_repo.get_or_create_current_session()?;
    assert_eq!(current_session_1.id, current_session_2.id);

    Ok(())
}

#[test]
fn test_must_not_return_init_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    assert!(gb_repo.get_current_session()?.is_none());

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

    Ok(())
}

#[test]
fn test_must_not_flush_without_current_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let session = gb_repo.flush()?;
    assert!(session.is_none());

    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

    Ok(())
}

#[test]
fn test_init_on_non_empty_repository() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);

    std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
    test_utils::commit_all(&repository);

    gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    Ok(())
}

#[test]
fn test_flush_on_existing_repository() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);

    std::fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
    test_utils::commit_all(&repository);

    gb_repository::Repository::open(
        gb_repo_path.clone(),
        &project.id,
        project_store.clone(),
        user_store.clone(),
    )?;

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    gb_repo.get_or_create_current_session()?;
    gb_repo.flush()?;

    Ok(())
}

#[test]
fn test_must_flush_current_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    gb_repo.get_or_create_current_session()?;

    let session = gb_repo.flush()?;
    assert!(session.is_some());
    let iter = gb_repo.get_sessions_iterator()?;
    assert_eq!(iter.count(), 1);

    Ok(())
}

#[test]
fn test_list_deltas_from_current_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let current_session = gb_repo.get_or_create_current_session()?;
    let writer = deltas::Writer::new(&gb_repo);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;

    let session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(
        deltas.get(&path::PathBuf::from("test.txt")).unwrap()[0]
            .operations
            .len(),
        1
    );
    assert_eq!(
        deltas.get(&path::PathBuf::from("test.txt")).unwrap()[0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_deltas_from_flushed_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let writer = deltas::Writer::new(&gb_repo);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session = gb_repo.flush()?;

    let session_reader = sessions::Reader::open(&gb_repo, &session.unwrap())?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(
        deltas.get(&path::PathBuf::from("test.txt")).unwrap()[0]
            .operations
            .len(),
        1
    );
    assert_eq!(
        deltas.get(&path::PathBuf::from("test.txt")).unwrap()[0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_files_from_current_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);

    // files are there before the session is created
    std::fs::write(
        repository.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let session = gb_repo.get_or_create_current_session()?;

    let reader = sessions::Reader::open(&gb_repo, &session)?;
    let files = reader.files(None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(
        files.get(&path::PathBuf::from("test.txt")).unwrap(),
        &reader::Content::UTF8("Hello World".to_string())
    );

    Ok(())
}

#[test]
fn test_list_files_from_flushed_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let user_store = users::Storage::from(&local_app_data);

    // files are there before the session is created
    std::fs::write(
        repository.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;

    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    gb_repo.get_or_create_current_session()?;
    let session = gb_repo.flush()?.unwrap();

    let reader = sessions::Reader::open(&gb_repo, &session)?;
    let files = reader.files(None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(
        files.get(&path::PathBuf::from("test.txt")).unwrap(),
        &reader::Content::UTF8("Hello World".to_string())
    );

    Ok(())
}

#[test]
fn test_remote_syncronization() -> Result<()> {
    // first, crate a remote, pretending it's a cloud
    let cloud = remote_repository()?;
    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: cloud.path().to_str().unwrap().to_string(),
        created_at: 0.to_string(),
        updated_at: 0.to_string(),
        sync: true,
    };

    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    let gb_repos_path = tempdir()?.path().to_str().unwrap().to_string();
    let user_store = users::Storage::from(&local_app_data);
    user_store.set(&users::User {
        name: "test".to_string(),
        email: "test@email.com".to_string(),
        ..Default::default()
    })?;

    // create first local project, add files, deltas and flush a session
    let repository_one = test_utils::test_repository();
    let project_one = projects::Project::try_from(&repository_one)?;
    project_store.add_project(&projects::Project {
        api: Some(api_project.clone()),
        ..project_one.clone()
    })?;
    std::fs::write(
        repository_one.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;
    let gb_repo_one = gb_repository::Repository::open(
        gb_repos_path.clone(),
        &project_one.id,
        project_store.clone(),
        user_store.clone(),
    )?;
    let writer = deltas::Writer::new(&gb_repo_one);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session_one = gb_repo_one.flush()?.unwrap();
    gb_repo_one.push().unwrap();

    // create second local project, fetch it and make sure session is there
    let repository_two = test_utils::test_repository();
    let project_two = projects::Project::try_from(&repository_two)?;
    project_store.add_project(&projects::Project {
        api: Some(api_project),
        ..project_two.clone()
    })?;
    let gb_repo_two =
        gb_repository::Repository::open(gb_repos_path, &project_two.id, project_store, user_store)?;
    gb_repo_two.fetch()?;
    // now it should have the session from the first local project synced
    let sessions_two = gb_repo_two
        .get_sessions_iterator()?
        .map(|s| s.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(sessions_two.len(), 1);
    assert_eq!(sessions_two[0].id, session_one.id);

    let session_reader = sessions::Reader::open(&gb_repo_two, &sessions_two[0])?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(None)?;
    let files = session_reader.files(None)?;
    assert_eq!(deltas.len(), 1);
    assert_eq!(files.len(), 1);
    assert_eq!(
        files.get(&path::PathBuf::from("test.txt")).unwrap(),
        &reader::Content::UTF8("Hello World".to_string())
    );
    assert_eq!(
        deltas.get(&path::PathBuf::from("test.txt")).unwrap(),
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }]
    );

    Ok(())
}

#[test]
fn test_remote_sync_order() -> Result<()> {
    // first, crate a remote, pretending it's a cloud
    let cloud = remote_repository()?;
    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: cloud.path().to_str().unwrap().to_string(),
        created_at: 0.to_string(),
        updated_at: 0.to_string(),
        sync: true,
    };

    let local_app_data = tempdir()?.path().to_path_buf();
    let project_store = projects::Storage::from(&local_app_data);
    let gb_repos_path = tempdir()?.path().to_str().unwrap().to_string();
    let user_store = users::Storage::from(&local_app_data);
    user_store.set(&users::User {
        name: "test".to_string(),
        email: "test@email.com".to_string(),
        ..Default::default()
    })?;

    // create first project and repo
    let repository_one = test_utils::test_repository();
    let project_one = projects::Project::try_from(&repository_one)?;
    project_store.add_project(&projects::Project {
        api: Some(api_project.clone()),
        ..project_one.clone()
    })?;
    let gb_repo_one = gb_repository::Repository::open(
        gb_repos_path.clone(),
        &project_one.id,
        project_store.clone(),
        user_store.clone(),
    )?;

    // create second project and repo
    let repository_two = test_utils::test_repository();
    let project_two = projects::Project::try_from(&repository_two)?;
    project_store.add_project(&projects::Project {
        api: Some(api_project),
        ..project_two.clone()
    })?;
    let gb_repo_two =
        gb_repository::Repository::open(gb_repos_path, &project_two.id, project_store, user_store)?;

    // create session in the first project
    gb_repo_one.get_or_create_current_session()?;
    std::fs::write(
        repository_one.path().parent().unwrap().join("test.txt"),
        "Hello World",
    )?;
    let session_one_first = gb_repo_one.flush()?.unwrap();
    gb_repo_one.push().unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create session in the second project
    gb_repo_two.get_or_create_current_session()?;
    std::fs::write(
        repository_two.path().parent().unwrap().join("test2.txt"),
        "Hello World",
    )?;
    let session_two_first = gb_repo_two.flush()?.unwrap();
    gb_repo_two.push().unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create second session in the first project
    gb_repo_one.get_or_create_current_session()?;
    std::fs::write(
        repository_one.path().parent().unwrap().join("test.txt"),
        "Hello World again",
    )?;
    let session_one_second = gb_repo_one.flush()?.unwrap();
    gb_repo_one.push().unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create second session in the second project
    gb_repo_two.get_or_create_current_session()?;
    std::fs::write(
        repository_two.path().parent().unwrap().join("test2.txt"),
        "Hello World again",
    )?;
    let session_two_second = gb_repo_two.flush()?.unwrap();
    gb_repo_two.push().unwrap();

    gb_repo_one.fetch()?;
    let sessions_one = gb_repo_one
        .get_sessions_iterator()?
        .map(|s| s.unwrap())
        .collect::<Vec<_>>();

    gb_repo_two.fetch()?;
    let sessions_two = gb_repo_two
        .get_sessions_iterator()?
        .map(|s| s.unwrap())
        .collect::<Vec<_>>();

    // make sure the sessions are the same on both repos
    assert_eq!(sessions_one.len(), 4);
    assert_eq!(sessions_two, sessions_one);

    assert_eq!(sessions_one[0].id, session_two_second.id);
    assert_eq!(sessions_one[1].id, session_one_second.id);
    assert_eq!(sessions_one[2].id, session_two_first.id);
    assert_eq!(sessions_one[3].id, session_one_first.id);

    Ok(())
}
