use anyhow::Result;
use tempfile::tempdir;

use crate::{
    app::{gb_repository, project_repository},
    deltas, projects, sessions, storage, users,
};

use super::project_file_change::Handler;

fn commit_all(repository: &git2::Repository) -> Result<git2::Oid> {
    let mut index = repository.index()?;
    index.add_all(&["."], git2::IndexAddOption::DEFAULT, None)?;
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
fn test_register_existing_commited_file() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    commit_all(&repository)?;

    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    std::fs::write(project_repo.root().join(file_path), "test2")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let session_reader = gb_repo.get_session_reader(&session)?;
    let deltas = session_reader.file_deltas("test.txt")?.unwrap();
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
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;

    listener.handle(file_path)?;

    assert!(gb_repo.get_current_session()?.is_some());

    Ok(())
}

#[test]
fn test_register_must_not_override_current_session() -> Result<()> {
    let repository = test_repository()?;
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

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
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;

    listener.handle(file_path)?;

    let sesssion = gb_repo.get_current_session()?.unwrap();
    let reader = gb_repo.get_session_reader(&sesssion)?;
    let deltas = reader.file_deltas("test.txt")?.unwrap();
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
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let reader = gb_repo.get_session_reader(&session)?;
    let deltas = reader.file_deltas("test.txt")?.unwrap();
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

    let deltas = reader.file_deltas("test.txt")?.unwrap();
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
    let project = test_project(&repository)?;
    let project_repo = project_repository::Repository::open(&project)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let file_path = std::path::Path::new("test.txt");
    std::fs::write(project_repo.root().join(file_path), "test")?;
    listener.handle(file_path)?;

    let session = gb_repo.get_current_session()?.unwrap();
    let reader = gb_repo.get_session_reader(&session)?;
    let deltas = reader.file_deltas("test.txt")?.unwrap();
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

    let deltas = reader.file_deltas("test.txt")?.unwrap();
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
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

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
        listener.handle(&relative_file_path)?;
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
            let reader = gb_repo.get_session_reader(&session).unwrap();
            let deltas_by_filepath = reader.deltas(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                deltas.iter().for_each(|delta| {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                });
            }
        });

        let reader = gb_repo
            .get_session_reader(&sessions_slice.first().unwrap())
            .unwrap();
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
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let size = 10;
    let relative_file_path = std::path::Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            std::path::Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.handle(&relative_file_path)?;
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
            let reader = gb_repo.get_session_reader(&session).unwrap();
            let deltas_by_filepath = reader.deltas(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                deltas.iter().for_each(|delta| {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                });
            }
        });

        let reader = gb_repo
            .get_session_reader(&sessions_slice.first().unwrap())
            .unwrap();
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
    let project = test_project(&repository)?;
    let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
    let storage = storage::Storage::from_path(tempdir()?.path().to_path_buf());
    let user_store = users::Storage::new(storage.clone());
    let project_store = projects::Storage::new(storage);
    project_store.add_project(&project)?;
    let gb_repo = gb_repository::Repository::open(
        gb_repo_path,
        project.id.clone(),
        project_store.clone(),
        user_store,
    )?;
    let listener = Handler::new(project.id.clone(), project_store, &gb_repo);

    let size = 10;
    let relative_file_path = std::path::Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            std::path::Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.handle(&relative_file_path)?;
    }

    // collect all operations from sessions in the reverse order
    let mut operations: Vec<deltas::Operation> = vec![];
    let session = gb_repo.get_current_session()?.unwrap();
    let reader = gb_repo.get_session_reader(&session).unwrap();
    let deltas_by_filepath = reader.deltas(None).unwrap();
    for deltas in deltas_by_filepath.values() {
        deltas.iter().for_each(|delta| {
            delta.operations.iter().for_each(|operation| {
                operations.push(operation.clone());
            });
        });
    }

    let reader = gb_repo.get_session_reader(&session).unwrap();
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
