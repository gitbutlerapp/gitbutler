use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;
use gitbutler_core::projects::ProjectId;
use gitbutler_core::{
    deltas::{self, operations::Operation},
    reader, sessions,
    virtual_branches::{self, branch, VirtualBranchesHandle},
};
use once_cell::sync::Lazy;

use self::branch::BranchId;
use crate::handler::support::Fixture;
use gitbutler_testsupport::{commit_all, Case};

static TEST_TARGET_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

#[derive(Clone)]
pub struct State {
    inner: gitbutler_watcher::Handler,
}

impl State {
    pub(super) fn from_fixture(fixture: &mut Fixture) -> Self {
        Self {
            inner: fixture.new_handler(),
        }
    }

    pub(super) fn calculate_delta(
        &self,
        path: impl Into<PathBuf>,
        project_id: ProjectId,
    ) -> Result<()> {
        self.inner.calculate_deltas(vec![path.into()], project_id)?;
        Ok(())
    }
}

fn new_test_target() -> virtual_branches::target::Target {
    virtual_branches::target::Target {
        branch: format!(
            "refs/remotes/remote name {}/branch name {}",
            TEST_TARGET_INDEX.load(Ordering::Relaxed),
            TEST_TARGET_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
        remote_url: format!("remote url {}", TEST_TARGET_INDEX.load(Ordering::Relaxed)),
        sha: format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            TEST_TARGET_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
    }
}

static TEST_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

fn new_test_branch() -> branch::Branch {
    TEST_INDEX.fetch_add(1, Ordering::Relaxed);

    branch::Branch {
        id: BranchId::generate(),
        name: format!("branch_name_{}", TEST_INDEX.load(Ordering::Relaxed)),
        notes: format!("branch_notes_{}", TEST_INDEX.load(Ordering::Relaxed)),
        applied: true,
        upstream: Some(
            format!(
                "refs/remotes/origin/upstream_{}",
                TEST_INDEX.load(Ordering::Relaxed)
            )
            .parse()
            .unwrap(),
        ),
        upstream_head: None,
        created_timestamp_ms: TEST_INDEX.load(Ordering::Relaxed) as u128,
        updated_timestamp_ms: (TEST_INDEX.load(Ordering::Relaxed) + 100) as u128,
        head: format!(
            "0123456789abcdef0123456789abcdef0123456{}",
            TEST_INDEX.load(Ordering::Relaxed)
        )
        .parse()
        .unwrap(),
        tree: format!(
            "0123456789abcdef0123456789abcdef012345{}",
            TEST_INDEX.load(Ordering::Relaxed) + 10
        )
        .parse()
        .unwrap(),
        ownership: branch::BranchOwnershipClaims::default(),
        order: TEST_INDEX.load(Ordering::Relaxed),
        selected_for_changes: None,
    }
}

#[test]
fn register_existing_commited_file() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

    std::fs::write(project.path.join("test.txt"), "test2")?;
    listener.calculate_delta("test.txt", project.id)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        Operation::Insert((4, "2".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
        "test2"
    );

    Ok(())
}

#[test]
fn register_must_init_current_session() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(project.path.join("test.txt"), "test")?;
    listener.calculate_delta("test.txt", project.id)?;

    assert!(gb_repository.get_current_session()?.is_some());

    Ok(())
}

#[test]
fn register_must_not_override_current_session() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(project.path.join("test.txt"), "test")?;
    listener.calculate_delta("test.txt", project.id)?;
    let session1 = gb_repository.get_current_session()?.unwrap();

    std::fs::write(project.path.join("test.txt"), "test2")?;
    listener.calculate_delta("test.txt", project.id)?;
    let session2 = gb_repository.get_current_session()?.unwrap();

    assert_eq!(session1.id, session2.id);

    Ok(())
}

#[test]
fn register_binfile() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(
        project.path.join("test.bin"),
        [0, 159, 146, 150, 159, 146, 150],
    )?;

    listener.calculate_delta("test.bin", project.id)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.bin")?.unwrap();

    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 0);
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.bin"))?,
        ""
    );

    Ok(())
}

#[test]
fn register_empty_new_file() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(project.path.join("test.txt"), "")?;

    listener.calculate_delta("test.txt", project.id)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 0);
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
        ""
    );

    Ok(())
}

#[test]
fn register_new_file() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(project.path.join("test.txt"), "test")?;

    listener.calculate_delta("test.txt", project.id)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
        "test"
    );

    Ok(())
}

#[test]
fn register_no_changes_saved_thgoughout_flushes() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project_repository,
        project,
        ..
    } = &fixture.new_case();

    // file change, wd and deltas are written
    std::fs::write(project.path.join("test.txt"), "test")?;
    listener.calculate_delta("test.txt", project.id)?;

    // make two more sessions.
    gb_repository.flush(project_repository, None)?;
    gb_repository.get_or_create_current_session()?;
    gb_repository.flush(project_repository, None)?;

    // after some sessions, files from the first change are still there.
    let session = gb_repository.get_or_create_current_session()?;
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let files = session_reader.files(None)?;
    assert_eq!(files.len(), 1);

    Ok(())
}

#[test]
fn register_new_file_twice() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    std::fs::write(project.path.join("test.txt"), "test")?;
    listener.calculate_delta("test.txt", project.id)?;

    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session)?;
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
        "test"
    );

    std::fs::write(project.path.join("test.txt"), "test2")?;
    listener.calculate_delta("test.txt", project.id)?;

    let deltas = deltas_reader.read_file("test.txt")?.unwrap();
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].operations.len(), 1);
    assert_eq!(
        deltas[0].operations[0],
        Operation::Insert((0, "test".to_string())),
    );
    assert_eq!(deltas[1].operations.len(), 1);
    assert_eq!(
        deltas[1].operations[0],
        Operation::Insert((4, "2".to_string())),
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
        "test2"
    );

    Ok(())
}

#[test]
fn register_file_deleted() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project_repository,
        project,
        ..
    } = &fixture.new_case();

    {
        // write file
        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.calculate_delta("test.txt", project.id)?;
    }

    {
        // current session must have the deltas, but not the file (it didn't exist)
        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(
            deltas[0].operations[0],
            Operation::Insert((0, "test".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test"
        );

        let files = session_reader.files(None).unwrap();
        assert!(files.is_empty());
    }

    gb_repository.flush(project_repository, None)?;

    {
        // file should be available in the next session, but not deltas just yet.
        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(gb_repository, &session)?;
        let files = session_reader.files(None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[Path::new("test.txt")],
            reader::Content::UTF8("test".to_string())
        );

        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read(None)?;
        assert!(deltas.is_empty());

        // removing the file
        std::fs::remove_file(project.path.join("test.txt"))?;
        listener.calculate_delta("test.txt", project.id)?;

        // deltas are recorded
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(deltas[0].operations[0], Operation::Delete((0, 4)),);
    }

    gb_repository.flush(project_repository, None)?;

    {
        // since file was deleted in the previous session, it should not exist in the new one.
        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(gb_repository, &session)?;
        let files = session_reader.files(None).unwrap();
        assert!(files.is_empty());
    }

    Ok(())
}

#[test]
fn flow_with_commits() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        project_repository,
        ..
    } = &fixture.new_case();

    let size = 10;
    let relative_file_path = Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        commit_all(&project_repository.git_repository);
        listener.calculate_delta(relative_file_path, project.id)?;
        assert!(gb_repository.flush(project_repository, None)?.is_some());
    }

    // get all the created sessions
    let mut sessions: Vec<sessions::Session> = gb_repository
        .get_sessions_iterator()?
        .map(Result::unwrap)
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
    for i in 0..sessions.len() {
        let sessions_slice = &mut sessions[i..];

        // collect all operations from sessions in the reverse order
        let mut operations: Vec<Operation> = vec![];
        for session in &mut *sessions_slice {
            let session_reader = sessions::Reader::open(gb_repository, session).unwrap();
            let deltas_reader = deltas::Reader::new(&session_reader);
            let deltas_by_filepath = deltas_reader.read(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                for delta in deltas {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                }
            }
        }

        let reader =
            sessions::Reader::open(gb_repository, sessions_slice.first().unwrap()).unwrap();
        let files = reader.files(None).unwrap();

        if i == 0 {
            assert_eq!(files.len(), 0);
        } else {
            assert_eq!(files.len(), 1);
        }

        let base_file = files.get(&relative_file_path.to_path_buf());
        let mut text: Vec<char> = match base_file {
            Some(reader::Content::UTF8(file)) => file.chars().collect(),
            _ => vec![],
        };

        for operation in operations {
            operation.apply(&mut text).unwrap();
        }

        assert_eq!(text.iter().collect::<String>(), size.to_string());
    }
    Ok(())
}

#[test]
fn flow_no_commits() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        project_repository,
        ..
    } = &fixture.new_case();

    let size = 10;
    let relative_file_path = Path::new("one/two/test.txt");
    for i in 1..=size {
        std::fs::create_dir_all(Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.calculate_delta(relative_file_path, project.id)?;
        assert!(gb_repository.flush(project_repository, None)?.is_some());
    }

    // get all the created sessions
    let mut sessions: Vec<sessions::Session> = gb_repository
        .get_sessions_iterator()?
        .map(Result::unwrap)
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
    for i in 0..sessions.len() {
        let sessions_slice = &mut sessions[i..];

        // collect all operations from sessions in the reverse order
        let mut operations: Vec<Operation> = vec![];
        for session in &mut *sessions_slice {
            let session_reader = sessions::Reader::open(gb_repository, session).unwrap();
            let deltas_reader = deltas::Reader::new(&session_reader);
            let deltas_by_filepath = deltas_reader.read(None).unwrap();
            for deltas in deltas_by_filepath.values() {
                for delta in deltas {
                    delta.operations.iter().for_each(|operation| {
                        operations.push(operation.clone());
                    });
                }
            }
        }

        let reader =
            sessions::Reader::open(gb_repository, sessions_slice.first().unwrap()).unwrap();
        let files = reader.files(None).unwrap();

        if i == 0 {
            assert_eq!(files.len(), 0);
        } else {
            assert_eq!(files.len(), 1);
        }

        let base_file = files.get(&relative_file_path.to_path_buf());
        let mut text: Vec<char> = match base_file {
            Some(reader::Content::UTF8(file)) => file.chars().collect(),
            _ => vec![],
        };

        for operation in operations {
            operation.apply(&mut text).unwrap();
        }

        assert_eq!(text.iter().collect::<String>(), size.to_string());
    }
    Ok(())
}

#[test]
fn flow_signle_session() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        ..
    } = &fixture.new_case();

    let size = 10_i32;
    let relative_file_path = Path::new("one/two/test.txt");
    for i in 1_i32..=size {
        std::fs::create_dir_all(Path::new(&project.path).join("one/two"))?;
        // create a session with a single file change and flush it
        std::fs::write(
            Path::new(&project.path).join(relative_file_path),
            i.to_string(),
        )?;

        listener.calculate_delta(relative_file_path, project.id)?;
    }

    // collect all operations from sessions in the reverse order
    let mut operations: Vec<Operation> = vec![];
    let session = gb_repository.get_current_session()?.unwrap();
    let session_reader = sessions::Reader::open(gb_repository, &session).unwrap();
    let deltas_reader = deltas::Reader::new(&session_reader);
    let deltas_by_filepath = deltas_reader.read(None).unwrap();
    for deltas in deltas_by_filepath.values() {
        for delta in deltas {
            delta.operations.iter().for_each(|operation| {
                operations.push(operation.clone());
            });
        }
    }

    let reader = sessions::Reader::open(gb_repository, &session).unwrap();
    let files = reader.files(None).unwrap();

    let base_file = files.get(&relative_file_path.to_path_buf());
    let mut text: Vec<char> = match base_file {
        Some(reader::Content::UTF8(file)) => file.chars().collect(),
        _ => vec![],
    };

    for operation in operations {
        operation.apply(&mut text).unwrap();
    }

    assert_eq!(text.iter().collect::<String>(), size.to_string());
    Ok(())
}

#[test]
fn should_persist_branches_targets_state_between_sessions() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        project_repository,
        ..
    } = &fixture.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "hello world")]));

    let branch_writer =
        branch::Writer::new(gb_repository, VirtualBranchesHandle::new(&project.gb_dir()))?;
    let target_writer = virtual_branches::target::Writer::new(
        gb_repository,
        VirtualBranchesHandle::new(&project.gb_dir()),
    )?;
    let default_target = new_test_target();
    target_writer.write_default(&default_target)?;
    let mut vbranch0 = new_test_branch();
    branch_writer.write(&mut vbranch0)?;
    let mut vbranch1 = new_test_branch();
    let vbranch1_target = new_test_target();
    branch_writer.write(&mut vbranch1)?;
    target_writer.write(&vbranch1.id, &vbranch1_target)?;

    std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
    listener.calculate_delta("test.txt", project.id)?;

    let flushed_session = gb_repository.flush(project_repository, None).unwrap();

    // create a new session
    let session = gb_repository.get_or_create_current_session().unwrap();
    assert_ne!(session.id, flushed_session.unwrap().id);

    // ensure that the virtual branch is still there and selected
    let session_reader = sessions::Reader::open(gb_repository, &session).unwrap();

    let branches = virtual_branches::Iterator::new(
        &session_reader,
        VirtualBranchesHandle::new(&project_repository.project().gb_dir()),
        project_repository.project().use_toml_vbranches_state(),
    )
    .unwrap()
    .collect::<Result<Vec<virtual_branches::Branch>, gitbutler_core::reader::Error>>()
    .unwrap()
    .into_iter()
    .collect::<Vec<virtual_branches::Branch>>();
    assert_eq!(branches.len(), 2);
    let branch_ids = branches.iter().map(|b| b.id).collect::<Vec<_>>();
    assert!(branch_ids.contains(&vbranch0.id));
    assert!(branch_ids.contains(&vbranch1.id));

    let target_reader = virtual_branches::target::Reader::new(
        &session_reader,
        VirtualBranchesHandle::new(&project_repository.project().gb_dir()),
        project_repository.project().use_toml_vbranches_state(),
    );
    assert_eq!(target_reader.read_default().unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

    Ok(())
}

#[test]
fn should_restore_branches_targets_state_from_head_session() -> Result<()> {
    let mut fixture = Fixture::default();
    let listener = State::from_fixture(&mut fixture);
    let Case {
        gb_repository,
        project,
        project_repository,
        ..
    } = &fixture.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "hello world")]));

    let branch_writer =
        branch::Writer::new(gb_repository, VirtualBranchesHandle::new(&project.gb_dir()))?;
    let target_writer = virtual_branches::target::Writer::new(
        gb_repository,
        VirtualBranchesHandle::new(&project.gb_dir()),
    )?;
    let default_target = new_test_target();
    target_writer.write_default(&default_target)?;
    let mut vbranch0 = new_test_branch();
    branch_writer.write(&mut vbranch0)?;
    let mut vbranch1 = new_test_branch();
    let vbranch1_target = new_test_target();
    branch_writer.write(&mut vbranch1)?;
    target_writer.write(&vbranch1.id, &vbranch1_target)?;

    std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
    listener.calculate_delta("test.txt", project.id).unwrap();

    let flushed_session = gb_repository.flush(project_repository, None).unwrap();

    // hard delete branches state from disk
    std::fs::remove_dir_all(gb_repository.root()).unwrap();

    // create a new session
    let session = gb_repository.get_or_create_current_session().unwrap();
    assert_ne!(session.id, flushed_session.unwrap().id);

    // ensure that the virtual branch is still there and selected
    let session_reader = sessions::Reader::open(gb_repository, &session).unwrap();

    let branches = virtual_branches::Iterator::new(
        &session_reader,
        VirtualBranchesHandle::new(&project_repository.project().gb_dir()),
        project_repository.project().use_toml_vbranches_state(),
    )
    .unwrap()
    .collect::<Result<Vec<virtual_branches::Branch>, gitbutler_core::reader::Error>>()
    .unwrap()
    .into_iter()
    .collect::<Vec<virtual_branches::Branch>>();
    assert_eq!(branches.len(), 2);
    let branch_ids = branches.iter().map(|b| b.id).collect::<Vec<_>>();
    assert!(branch_ids.contains(&vbranch0.id));
    assert!(branch_ids.contains(&vbranch1.id));

    let target_reader = virtual_branches::target::Reader::new(
        &session_reader,
        VirtualBranchesHandle::new(&project_repository.project().gb_dir()),
        project_repository.project().use_toml_vbranches_state(),
    );
    assert_eq!(target_reader.read_default().unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
    assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

    Ok(())
}

mod flush_wd {
    use super::*;

    #[test]
    fn should_add_new_files_to_session_wd() {
        let mut fixture = Fixture::default();
        let listener = State::from_fixture(&mut fixture);
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = &fixture.new_case();

        // write a file into session
        std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
        listener.calculate_delta("test.txt", project.id).unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush it should be flushed into the commit
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert_eq!(
                commit_reader.list_files(Path::new("wd")).unwrap(),
                vec![Path::new("test.txt")]
            );
            assert_eq!(
                commit_reader.read(Path::new("wd/test.txt")).unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
        }

        // write another file into session
        std::fs::create_dir_all(project.path.join("one/two")).unwrap();
        std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
        listener
            .calculate_delta("one/two/test2.txt", project.id)
            .unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush, it should be flushed into the commit next to the previous one
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert_eq!(
                commit_reader.list_files(Path::new("wd")).unwrap(),
                vec![Path::new("one/two/test2.txt"), Path::new("test.txt"),]
            );
            assert_eq!(
                commit_reader.read(Path::new("wd/test.txt")).unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
            assert_eq!(
                commit_reader
                    .read(Path::new("wd/one/two/test2.txt"))
                    .unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
        }
    }

    #[test]
    fn should_remove_deleted_files_from_session_wd() {
        let mut fixture = Fixture::default();
        let listener = State::from_fixture(&mut fixture);
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = &fixture.new_case();

        // write a file into session
        std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
        listener.calculate_delta("test.txt", project.id).unwrap();
        std::fs::create_dir_all(project.path.join("one/two")).unwrap();
        std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
        listener
            .calculate_delta("one/two/test2.txt", project.id)
            .unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush it should be flushed into the commit
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert_eq!(
                commit_reader.list_files(Path::new("wd")).unwrap(),
                vec![Path::new("one/two/test2.txt"), Path::new("test.txt"),]
            );
            assert_eq!(
                commit_reader.read(Path::new("wd/test.txt")).unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
            assert_eq!(
                commit_reader
                    .read(Path::new("wd/one/two/test2.txt"))
                    .unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
        }

        // rm the files
        std::fs::remove_file(project.path.join("test.txt")).unwrap();
        listener.calculate_delta("test.txt", project.id).unwrap();
        std::fs::remove_file(project.path.join("one/two/test2.txt")).unwrap();
        listener
            .calculate_delta("one/two/test2.txt", project.id)
            .unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush it should be removed from the commit
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert!(commit_reader
                .list_files(Path::new("wd"))
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn should_update_updated_files_in_session_wd() {
        let mut fixture = Fixture::default();
        let listener = State::from_fixture(&mut fixture);
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = &fixture.new_case();

        // write a file into session
        std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
        listener.calculate_delta("test.txt", project.id).unwrap();
        std::fs::create_dir_all(project.path.join("one/two")).unwrap();
        std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
        listener
            .calculate_delta("one/two/test2.txt", project.id)
            .unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush it should be flushed into the commit
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert_eq!(
                commit_reader.list_files(Path::new("wd")).unwrap(),
                vec![Path::new("one/two/test2.txt"), Path::new("test.txt"),]
            );
            assert_eq!(
                commit_reader.read(Path::new("wd/test.txt")).unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
            assert_eq!(
                commit_reader
                    .read(Path::new("wd/one/two/test2.txt"))
                    .unwrap(),
                reader::Content::UTF8("hello world!".to_string())
            );
        }

        // update the file
        std::fs::write(project.path.join("test.txt"), "hello world!2").unwrap();
        listener.calculate_delta("test.txt", project.id).unwrap();

        std::fs::write(project.path.join("one/two/test2.txt"), "hello world!2").unwrap();
        listener
            .calculate_delta("one/two/test2.txt", project.id)
            .unwrap();

        let flushed_session = gb_repository
            .flush(project_repository, None)
            .unwrap()
            .unwrap();
        {
            // after flush it should be updated in the commit
            let session_commit = gb_repository
                .git_repository()
                .find_commit(flushed_session.hash.unwrap())
                .unwrap();
            let commit_reader =
                reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                    .unwrap();
            assert_eq!(
                commit_reader.list_files(Path::new("wd")).unwrap(),
                vec![Path::new("one/two/test2.txt"), Path::new("test.txt"),]
            );
            assert_eq!(
                commit_reader.read(Path::new("wd/test.txt")).unwrap(),
                reader::Content::UTF8("hello world!2".to_string())
            );
            assert_eq!(
                commit_reader
                    .read(Path::new("wd/one/two/test2.txt"))
                    .unwrap(),
                reader::Content::UTF8("hello world!2".to_string())
            );
        }
    }
}
