use std::{collections::HashMap, path, thread, time};

use anyhow::Result;
use pretty_assertions::assert_eq;

use crate::{
    deltas,
    projects::{self, ProjectId},
    reader,
    sessions::{self, SessionId},
    test_utils::{Case, Suite},
};

fn test_remote_repository() -> Result<git2::Repository> {
    let path = tempfile::tempdir()?.path().to_str().unwrap().to_string();
    let repo_a = git2::Repository::init_bare(path)?;
    Ok(repo_a)
}

#[test]
fn test_get_current_session_writer_should_use_existing_session() -> Result<()> {
    let Case { gb_repository, .. } = Suite::default().new_case();

    let current_session_1 = gb_repository.get_or_create_current_session()?;
    let current_session_2 = gb_repository.get_or_create_current_session()?;
    assert_eq!(current_session_1.id, current_session_2.id);

    Ok(())
}

#[test]
fn test_must_not_return_init_session() -> Result<()> {
    let Case { gb_repository, .. } = Suite::default().new_case();

    assert!(gb_repository.get_current_session()?.is_none());

    let iter = gb_repository.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

    Ok(())
}

#[test]
fn test_must_not_flush_without_current_session() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default().new_case();

    let session = gb_repository.flush(&project_repository, None)?;
    assert!(session.is_none());

    let iter = gb_repository.get_sessions_iterator()?;
    assert_eq!(iter.count(), 0);

    Ok(())
}

#[test]
fn test_non_empty_repository() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default()
        .new_case_with_files(HashMap::from([(path::PathBuf::from("test.txt"), "test")]));

    gb_repository.get_or_create_current_session()?;
    gb_repository.flush(&project_repository, None)?;

    Ok(())
}

#[test]
fn test_must_flush_current_session() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default().new_case();

    gb_repository.get_or_create_current_session()?;

    let session = gb_repository.flush(&project_repository, None)?;
    assert!(session.is_some());

    let iter = gb_repository.get_sessions_iterator()?;
    assert_eq!(iter.count(), 1);

    Ok(())
}

#[test]
fn test_list_deltas_from_current_session() -> Result<()> {
    let Case { gb_repository, .. } = Suite::default().new_case();

    let current_session = gb_repository.get_or_create_current_session()?;
    let writer = deltas::Writer::new(&gb_repository);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;

    let session_reader = sessions::Reader::open(&gb_repository, &current_session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(&None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(
        deltas[&path::PathBuf::from("test.txt")][0].operations.len(),
        1
    );
    assert_eq!(
        deltas[&path::PathBuf::from("test.txt")][0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_deltas_from_flushed_session() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default().new_case();

    let writer = deltas::Writer::new(&gb_repository);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session = gb_repository.flush(&project_repository, None)?;

    let session_reader = sessions::Reader::open(&gb_repository, &session.unwrap())?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(&None)?;

    assert_eq!(deltas.len(), 1);
    assert_eq!(
        deltas[&path::PathBuf::from("test.txt")][0].operations.len(),
        1
    );
    assert_eq!(
        deltas[&path::PathBuf::from("test.txt")][0].operations[0],
        deltas::Operation::Insert((0, "Hello World".to_string()))
    );

    Ok(())
}

#[test]
fn test_list_files_from_current_session() -> Result<()> {
    let Case { gb_repository, .. } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "Hello World",
    )]));

    let current = gb_repository.get_or_create_current_session()?;
    let reader = sessions::Reader::open(&gb_repository, &current)?;
    let files = reader.files(&None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(
        files[&path::PathBuf::from("test.txt")],
        reader::Content::UTF8("Hello World".to_string())
    );

    Ok(())
}

#[test]
fn test_list_files_from_flushed_session() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default().new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "Hello World",
    )]));

    gb_repository.get_or_create_current_session()?;
    let session = gb_repository.flush(&project_repository, None)?.unwrap();
    let reader = sessions::Reader::open(&gb_repository, &session)?;
    let files = reader.files(&None)?;

    assert_eq!(files.len(), 1);
    assert_eq!(
        files[&path::PathBuf::from("test.txt")],
        reader::Content::UTF8("Hello World".to_string())
    );

    Ok(())
}

#[test]
fn test_remote_syncronization() -> Result<()> {
    // first, crate a remote, pretending it's a cloud
    let cloud = test_remote_repository()?;
    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: cloud.path().to_str().unwrap().to_string(),
        code_git_url: None,
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    let suite = Suite::default();
    let user = suite.sign_in();

    // create first local project, add files, deltas and flush a session
    let case_one = suite.new_case_with_files(HashMap::from([(
        path::PathBuf::from("test.txt"),
        "Hello World",
    )]));
    suite.projects.update(&projects::UpdateRequest {
        id: case_one.project.id,
        api: Some(api_project.clone()),
        ..Default::default()
    })?;
    let case_one = case_one.refresh();

    let writer = deltas::Writer::new(&case_one.gb_repository);
    writer.write(
        "test.txt",
        &vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }],
    )?;
    let session_one = case_one
        .gb_repository
        .flush(&case_one.project_repository, Some(&user))?
        .unwrap();
    case_one.gb_repository.push(Some(&user)).unwrap();

    // create second local project, fetch it and make sure session is there
    let case_two = suite.new_case();
    suite.projects.update(&projects::UpdateRequest {
        id: case_two.project.id,
        api: Some(api_project.clone()),
        ..Default::default()
    })?;
    let case_two = case_two.refresh();

    case_two.gb_repository.fetch(Some(&user))?;

    // now it should have the session from the first local project synced
    let sessions_two = case_two
        .gb_repository
        .get_sessions_iterator()?
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(sessions_two.len(), 1);
    assert_eq!(sessions_two[0].id, session_one.id);

    let session_reader = sessions::Reader::open(&case_two.gb_repository, &sessions_two[0])?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read(&None)?;
    let files = session_reader.files(&None)?;
    assert_eq!(deltas.len(), 1);
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[&path::PathBuf::from("test.txt")],
        reader::Content::UTF8("Hello World".to_string())
    );
    assert_eq!(
        deltas[&path::PathBuf::from("test.txt")],
        vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "Hello World".to_string()))],
            timestamp_ms: 0,
        }]
    );

    Ok(())
}

#[test]
fn test_remote_sync_order() -> Result<()> {
    // first, crate a remote, pretending it's a cloud
    let cloud = test_remote_repository()?;
    let api_project = projects::ApiProject {
        name: "test-sync".to_string(),
        description: None,
        repository_id: "123".to_string(),
        git_url: cloud.path().to_str().unwrap().to_string(),
        code_git_url: None,
        created_at: 0_i32.to_string(),
        updated_at: 0_i32.to_string(),
        sync: true,
    };

    let suite = Suite::default();

    let case_one = suite.new_case();
    suite.projects.update(&projects::UpdateRequest {
        id: case_one.project.id,
        api: Some(api_project.clone()),
        ..Default::default()
    })?;
    let case_one = case_one.refresh();

    let case_two = suite.new_case();
    suite.projects.update(&projects::UpdateRequest {
        id: case_two.project.id,
        api: Some(api_project.clone()),
        ..Default::default()
    })?;
    let case_two = case_two.refresh();

    let user = suite.sign_in();

    // create session in the first project
    case_one.gb_repository.get_or_create_current_session()?;
    let session_one_first = case_one
        .gb_repository
        .flush(&case_one.project_repository, Some(&user))?
        .unwrap();
    case_one.gb_repository.push(Some(&user)).unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create session in the second project
    case_two.gb_repository.get_or_create_current_session()?;
    let session_two_first = case_two
        .gb_repository
        .flush(&case_two.project_repository, Some(&user))?
        .unwrap();
    case_two.gb_repository.push(Some(&user)).unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create second session in the first project
    case_one.gb_repository.get_or_create_current_session()?;
    let session_one_second = case_one
        .gb_repository
        .flush(&case_one.project_repository, Some(&user))?
        .unwrap();
    case_one.gb_repository.push(Some(&user)).unwrap();

    thread::sleep(time::Duration::from_secs(1));

    // create second session in the second project
    case_two.gb_repository.get_or_create_current_session()?;
    let session_two_second = case_two
        .gb_repository
        .flush(&case_two.project_repository, Some(&user))?
        .unwrap();
    case_two.gb_repository.push(Some(&user)).unwrap();

    case_one.gb_repository.fetch(Some(&user))?;
    let sessions_one = case_one
        .gb_repository
        .get_sessions_iterator()?
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    case_two.gb_repository.fetch(Some(&user))?;
    let sessions_two = case_two
        .gb_repository
        .get_sessions_iterator()?
        .map(Result::unwrap)
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

#[test]
fn test_gitbutler_file() -> Result<()> {
    let Case {
        gb_repository,
        project_repository,
        ..
    } = Suite::default().new_case();

    let session = gb_repository.get_or_create_current_session()?;

    let gitbutler_file_path = project_repository.path().join(".git/gitbutler.json");
    assert!(gitbutler_file_path.exists());

    let file_content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&gitbutler_file_path)?)?;
    let sid: SessionId = file_content["sessionId"].as_str().unwrap().parse()?;
    assert_eq!(sid, session.id);

    let pid: ProjectId = file_content["repositoryId"].as_str().unwrap().parse()?;
    assert_eq!(pid, project_repository.project().id);

    Ok(())
}
