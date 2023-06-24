use anyhow::Result;
use tempfile::tempdir;

use crate::{
    deltas, gb_repository, project_repository, projects, sessions, storage, users,
    virtual_branches::{self, branch},
};

use super::project_file_change::Handler;

static mut TEST_TARGET_INDEX: usize = 0;

fn test_target() -> virtual_branches::target::Target {
    virtual_branches::target::Target {
        name: format!("target_name_{}", unsafe { TEST_TARGET_INDEX }),
        remote: format!("remote_{}", unsafe { TEST_TARGET_INDEX }),
        sha: git2::Oid::from_str(&format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            unsafe { TEST_TARGET_INDEX }
        ))
        .unwrap(),
        behind: 0,
    }
}

static mut TEST_INDEX: usize = 0;

fn test_branch() -> virtual_branches::branch::Branch {
    unsafe {
        TEST_INDEX += 1;
    }
    virtual_branches::branch::Branch {
        id: format!("branch_{}", unsafe { TEST_INDEX }),
        name: format!("branch_name_{}", unsafe { TEST_INDEX }),
        applied: true,
        upstream: format!("upstream_{}", unsafe { TEST_INDEX }),
        created_timestamp_ms: unsafe { TEST_INDEX } as u128,
        updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
        head: git2::Oid::from_str(&format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            unsafe { TEST_INDEX }
        ))
        .unwrap(),
        tree: git2::Oid::from_str(&format!(
            "0123456789abcdef0123456789abcdef012345{}",
            unsafe { TEST_INDEX + 10 }
        ))
        .unwrap(),
        ownership: vec![branch::Ownership {
            file_path: format!("file/{}", unsafe { TEST_INDEX }).into(),
            ranges: vec![],
        }],
    }
}

fn commit_all(repository: &git2::Repository) -> Result<git2::Oid> {
    let mut index = repository.index()?;
    index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index.write_tree()?;
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let commit_oid = repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "some commit",
        &repository.find_tree(oid)?,
        &[&repository.find_commit(repository.refname_to_id("HEAD")?)?],
    )?;
    Ok(commit_oid)
}

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

#[test]
fn test_register_existing_commited_file() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    std::fs::write(project_repo.root().join(file_path), "test2")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(&gb_repo, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((4, "2".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_wd_path().join(file_path))?,
        "test2"
    );

    Ok(())
}

#[test]
fn test_register_must_init_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;

    listener.handle(file_path)?;

    assert!(gb_repo.get_current_session()?.is_some());

    Ok(())
}

#[test]
fn test_register_must_not_override_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    listener.handle(file_path)?;

    let session1 = gb_repo.get_current_session()?.unwrap();

    std::fs::write(project_repo.root().join(file_path), "test2")?;
    listener.handle(file_path)?;

    let session2 = gb_repo.get_current_session()?.unwrap();
    assert_eq!(session1.id, session2.id);

    Ok(())
}

#[test]
fn test_register_new_file() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;

    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(&gb_repo, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_wd_path().join(file_path))?,
        "test"
    );

    Ok(())
}

#[test]
fn test_register_new_file_twice() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(&gb_repo, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_wd_path().join(file_path))?,
        "test"
    );

    std::fs::write(project_repo.root().join(file_path), "test2")?;
    listener.handle(file_path)?;

    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(deltas[1].operations.len(), 1);
    assert_eq!(
        deltas[1].operations[0],
        deltas::Operation::Insert((4, "2".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_wd_path().join(file_path))?,
        "test2"
    );

    Ok(())
}

#[test]
fn test_register_file_delted() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(&gb_repo, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_wd_path().join(file_path))?,
        "test"
    );

    std::fs::remove_file(project_repo.root().join(file_path))?;
    listener.handle(file_path)?;

    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        deltas::Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(deltas[1].operations.len(), 1);
    assert_eq!(deltas[1].operations[0], deltas::Operation::Delete((0, 4)),);

    Ok(())
}

#[test]
fn test_flow_with_commits() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let size = 10;
    let relative_file_path = std::path::Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            std::path::Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        commit_all(&repository)?;
        listener.handle(relative_file_path)?;
        assert!(gb_repo.flush()?.is_some());
    }

    // get all the created sessions
    let mut sessions: Vec<sessions::Session> = gb_repo
        .get_sessions_iterator()?
        .map(|s| s.unwrap())
        .collect();
    assert_eq!(sessions.len(), size);
    // verify sessions order is correct
    let mut last_start = sessions[0].meta.start_timestamp_ms;
    let mut last_end = sessions[0].meta.start_timestamp_ms;
    sessions[1..].iter().for_each(|session| {
        assert!(session.meta.start_timestamp_ms < last_start);
        assert!(session.meta.last_timestamp_ms < last_end);
        last_start = session.meta.start_timestamp_ms;
        last_end = session.meta.last_timestamp_ms;
    });

    sessions.reverse();
    // try to reconstruct file state from operations for every session slice
    for i in 0..=sessions.len() - 1 {
        let sessions_slice = &mut sessions[i..];

        // collect all operations from sessions in the reverse order
        let mut operations: Vec<deltas::Operation> = vec![];
        sessions_slice.iter().for_each(|session| {
            let session_reader = sessions::Reader::open(&gb_repo, session).unwrap();
            let deltas_reader = deltas::Reader::new(&session_reader);
            let deltas_by_filepath = deltas_reader.read(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                deltas.iter().for_each(|delta| {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                });
            }
        });

        let reader = sessions::Reader::open(&gb_repo, sessions_slice.first().unwrap()).unwrap();
        let files = reader.files(None).unwrap();

        if i == 0 {
            assert_eq!(files.len(), 0);
        } else {
            assert_eq!(files.len(), 1);
        }

        let base_file = files.get(&relative_file_path.to_str().unwrap().to_string());
        let mut text: Vec<char> = match base_file {
            Some(file) => file.chars().collect(),
            None => vec![],
        };

        for operation in operations {
            operation.apply(&mut text).unwrap();
        }

        assert_eq!(text.iter().collect::<String>(), size.to_string());
    }
    Ok(())
}

#[test]
fn test_flow_no_commits() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let size = 10;
    let relative_file_path = std::path::Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            std::path::Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.handle(relative_file_path)?;
        assert!(gb_repo.flush()?.is_some());
    }

    // get all the created sessions
    let mut sessions: Vec<sessions::Session> = gb_repo
        .get_sessions_iterator()?
        .map(|s| s.unwrap())
        .collect();
    assert_eq!(sessions.len(), size);
    // verify sessions order is correct
    let mut last_start = sessions[0].meta.start_timestamp_ms;
    let mut last_end = sessions[0].meta.start_timestamp_ms;
    sessions[1..].iter().for_each(|session| {
        assert!(session.meta.start_timestamp_ms < last_start);
        assert!(session.meta.last_timestamp_ms < last_end);
        last_start = session.meta.start_timestamp_ms;
        last_end = session.meta.last_timestamp_ms;
    });

    sessions.reverse();
    // try to reconstruct file state from operations for every session slice
    for i in 0..=sessions.len() - 1 {
        let sessions_slice = &mut sessions[i..];

        // collect all operations from sessions in the reverse order
        let mut operations: Vec<deltas::Operation> = vec![];
        sessions_slice.iter().for_each(|session| {
            let session_reader = sessions::Reader::open(&gb_repo, session).unwrap();
            let deltas_reader = deltas::Reader::new(&session_reader);
            let deltas_by_filepath = deltas_reader.read(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                deltas.iter().for_each(|delta| {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                });
            }
        });

        let reader = sessions::Reader::open(&gb_repo, sessions_slice.first().unwrap()).unwrap();
        let files = reader.files(None).unwrap();

        if i == 0 {
            assert_eq!(files.len(), 0);
        } else {
            assert_eq!(files.len(), 1);
        }

        let base_file = files.get(&relative_file_path.to_str().unwrap().to_string());
        let mut text: Vec<char> = match base_file {
            Some(file) => file.chars().collect(),
            None => vec![],
        };

        for operation in operations {
            operation.apply(&mut text).unwrap();
        }

        assert_eq!(text.iter().collect::<String>(), size.to_string());
    }
    Ok(())
}

#[test]
fn test_flow_signle_session() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let size = 10;
    let relative_file_path = std::path::Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            std::path::Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.handle(relative_file_path)?;
    }

    // collect all operations from sessions in the reverse order
    let mut operations: Vec<deltas::Operation> = vec![];
    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(&gb_repo, &session).unwrap();
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas_by_filepath = deltas_reader.read(None).unwrap();
    for deltas in deltas_by_filepath.values() {
        deltas.iter().for_each(|delta| {
            delta.operations.iter().for_each(|operation| {
                operations.push(operation.clone());
            });
        });
    }

    let reader = sessions::Reader::open(&gb_repo, &session).unwrap();
    let files = reader.files(None).unwrap();

    let base_file = files.get(&relative_file_path.to_str().unwrap().to_string());
    let mut text: Vec<char> = match base_file {
        Some(file) => file.chars().collect(),
        None => vec![],
    };

    for operation in operations {
        operation.apply(&mut text).unwrap();
    }

    assert_eq!(text.iter().collect::<String>(), size.to_string());
    Ok(())
}

#[test]
fn should_persist_branches_targets_state_between_sessions() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "hello world")?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let branch_writer = virtual_branches::branch::Writer::new(&gb_repo);
    let target_writer = virtual_branches::target::Writer::new(&gb_repo);
    let default_target = test_target();
    target_writer.write_default(&default_target)?;
    let vbranch0 = test_branch();
    branch_writer.write(&vbranch0)?;
    branch_writer.write_selected(&Some(vbranch0.id.clone()))?;
    let vbranch1 = test_branch();
    let vbranch1_target = test_target();
    branch_writer.write(&vbranch1)?;
    target_writer.write(&vbranch1.id, &vbranch1_target)?;

    std::fs::write(project_repo.root().join(file_path), "hello world!").unwrap();
    listener.handle(file_path)?;

    let flushed_session = gb_repo.flush().unwrap();

    // create a new session
    let session = gb_repo.get_or_create_current_session().unwrap();
    assert_ne!(session.id, flushed_session.unwrap().id);

    // ensure that the virtual branch is still there and selected
    let session_reader = sessions::Reader::open(&gb_repo, &session).unwrap();

    let branches = virtual_branches::Iterator::new(&session_reader)
        .unwrap()
        .collect::<Result<Vec<virtual_branches::Branch>, crate::reader::Error>>()
        .unwrap()
        .into_iter()
        .collect::<Vec<virtual_branches::Branch>>();
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].id, vbranch0.id);
    assert_eq!(branches[1].id, vbranch1.id);

    let vbranch_reader = virtual_branches::branch::Reader::new(&session_reader);
    let selected = vbranch_reader.read_selected().unwrap().unwrap();
    assert_eq!(selected, vbranch0.id);

    let target_reader = virtual_branches::target::Reader::new(&session_reader);
    assert_eq!(target_reader.read_default().unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

    Ok(())
}

#[test]
fn should_restore_branches_targets_state_from_head_session() -> Result<()> {
    let repository = test_repository()?;
    let project = projects::Project::try_from(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    println!("{}", gb_repo_path);

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "hello world")?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path.clone(),
        project.id.clone(),
        project_store.clone(),
        user_store.clone(),
    )?;
    let listener = Handler::new(
        gb_repo_path.into(),
        project.id.clone(),
        project_store,
        user_store,
    );

    let branch_writer = virtual_branches::branch::Writer::new(&gb_repo);
    let target_writer = virtual_branches::target::Writer::new(&gb_repo);
    let default_target = test_target();
    target_writer.write_default(&default_target)?;
    let vbranch0 = test_branch();
    branch_writer.write(&vbranch0)?;
    branch_writer.write_selected(&Some(vbranch0.id.clone()))?;
    let vbranch1 = test_branch();
    let vbranch1_target = test_target();
    branch_writer.write(&vbranch1)?;
    target_writer.write(&vbranch1.id, &vbranch1_target)?;

    std::fs::write(project_repo.root().join(file_path), "hello world!").unwrap();
    listener.handle(file_path).unwrap();

    let flushed_session = gb_repo.flush().unwrap();

    // hard delete branches state from disk
    std::fs::remove_dir_all(gb_repo.root()).unwrap();

    // create a new session
    let session = gb_repo.get_or_create_current_session().unwrap();
    assert_ne!(session.id, flushed_session.unwrap().id);

    // ensure that the virtual branch is still there and selected
    let session_reader = sessions::Reader::open(&gb_repo, &session).unwrap();

    let branches = virtual_branches::Iterator::new(&session_reader)
        .unwrap()
        .collect::<Result<Vec<virtual_branches::Branch>, crate::reader::Error>>()
        .unwrap()
        .into_iter()
        .collect::<Vec<virtual_branches::Branch>>();
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].id, vbranch0.id);
    assert_eq!(branches[1].id, vbranch1.id);

    let vbranch_reader = virtual_branches::branch::Reader::new(&session_reader);
    let selected = vbranch_reader.read_selected().unwrap().unwrap();
    assert_eq!(selected, vbranch0.id);

    let target_reader = virtual_branches::target::Reader::new(&session_reader);
    assert_eq!(target_reader.read_default().unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

    Ok(())
}
