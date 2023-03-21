use crate::{deltas::Operation, projects, repositories, sessions};
use anyhow::Result;
use std::path::Path;
use tempfile::tempdir;

fn test_project() -> Result<repositories::Repository> {
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
    repositories::Repository::new(project.clone(), None)
}

#[test]
fn test_flush_session() {
    let repo = test_project().unwrap();
    let sessions_store =
        sessions::Store::new(clone_repo(&repo.git_repository), repo.project.clone()).unwrap();

    let relative_file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&repo.project.path).join(relative_file_path),
        "hello",
    )
    .unwrap();
    let git_repo = repo.git_repository;

    let result = super::delta::register_file_change(
        &repo.project,
        &git_repo,
        &repo.deltas_storage,
        &relative_file_path,
    );
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (mut session1, deltas1) = maybe_session_deltas.unwrap();
    assert_eq!(session1.hash, None);
    assert_eq!(deltas1.len(), 1);

    session1 = sessions_store.flush(&session1, None).unwrap();
    assert!(session1.hash.is_some());

    std::fs::write(
        Path::new(&repo.project.path).join(relative_file_path),
        "hello world",
    )
    .unwrap();

    let result = super::delta::register_file_change(
        &repo.project,
        &git_repo,
        &repo.deltas_storage,
        &relative_file_path,
    );
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (mut session2, deltas2) = maybe_session_deltas.unwrap();
    assert_eq!(session2.hash, None);
    assert_eq!(deltas2.len(), 1);
    assert_ne!(session1.id, session2.id);

    session2 = sessions_store.flush(&session2, None).unwrap();
    assert!(session2.hash.is_some());
}

fn clone_repo(repo: &git2::Repository) -> git2::Repository {
    git2::Repository::open(repo.path()).unwrap()
}

#[test]
fn test_flow() {
    let repo = test_project().unwrap();
    let sessions_store =
        sessions::Store::new(clone_repo(&repo.git_repository), repo.project.clone()).unwrap();

    let size = 10;
    let relative_file_path = Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(Path::new(&repo.project.path).join("one/two")).unwrap();
        // create a session with a single file change and flush it
        std::fs::write(
            Path::new(&repo.project.path).join(relative_file_path),
            i.to_string(),
        )
        .unwrap();

        let result = super::delta::register_file_change(
            &repo.project,
            &repo.git_repository,
            &repo.deltas_storage,
            &relative_file_path,
        );
        assert!(result.is_ok());
        let maybe_session_deltas = result.unwrap();
        assert!(maybe_session_deltas.is_some());
        let (mut session, deltas) = maybe_session_deltas.unwrap();
        assert_eq!(session.hash, None);
        assert_eq!(deltas.len(), 1);

        session = sessions_store.flush(&session, None).unwrap();
        assert!(session.hash.is_some());
    }

    // get all the created sessions
    let mut sessions = sessions_store.list(None).unwrap();
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
        let mut operations: Vec<Operation> = vec![];
        sessions_slice.iter().for_each(|session| {
            let deltas_by_filepath = repo.deltas(&session.id, None).unwrap();
            for deltas in deltas_by_filepath.values() {
                deltas.iter().for_each(|delta| {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                });
            }
        });

        let files = repo
            .files(&sessions_slice.first().unwrap().id, None)
            .unwrap();

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
}
