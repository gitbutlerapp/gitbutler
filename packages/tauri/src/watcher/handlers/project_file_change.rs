use std::{path, vec};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    deltas, gb_repository,
    paths::DataDir,
    project_repository,
    projects::{self, ProjectId},
    reader::{self, Reader},
    sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
}

impl From<&DataDir> for Handler {
    fn from(value: &DataDir) -> Self {
        Self {
            local_data_dir: value.clone(),
            projects: projects::Controller::from(value),
            users: users::Controller::from(value),
        }
    }
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::try_from(value)?,
        })
    }
}

impl Handler {
    // Returns Some(file_content) or None if the file is ignored.
    fn get_current_file(
        project_repository: &project_repository::Repository,
        path: &std::path::Path,
    ) -> Result<reader::Content, reader::Error> {
        if project_repository.is_path_ignored(path).unwrap_or(false) {
            return Err(reader::Error::NotFound);
        }

        let reader = project_repository.get_wd_reader();

        reader.read(path)
    }

    // returns deltas for the file that are already part of the current session (if any)
    fn get_current_deltas(
        gb_repo: &gb_repository::Repository,
        path: &path::Path,
    ) -> Result<Option<Vec<deltas::Delta>>> {
        let current_session = gb_repo.get_current_session()?;
        if current_session.is_none() {
            return Ok(None);
        }
        let current_session = current_session.unwrap();
        let session_reader = sessions::Reader::open(gb_repo, &current_session)
            .context("failed to get session reader")?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader
            .read_file(path)
            .context("failed to get file deltas")?;
        Ok(deltas)
    }

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;

        let project_repository = project_repository::Repository::try_from(&project)
            .with_context(|| "failed to open project repository for project")?;

        let user = self.users.get_user().context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repository")?;

        // If current session's branch is not the same as the project's head, flush it first.
        if let Some(session) = gb_repository
            .get_current_session()
            .context("failed to get current session")?
        {
            let project_head = project_repository
                .get_head()
                .context("failed to get head")?;
            if session.meta.branch != project_head.name().map(ToString::to_string) {
                gb_repository
                    .flush_session(&project_repository, &session, user.as_ref())
                    .context("failed to flush session")?;
            }
        }

        let path = path.as_ref();

        let current_wd_file_content = match Self::get_current_file(&project_repository, path) {
            Ok(content) => Some(content),
            Err(reader::Error::NotFound) => None,
            Err(err) => Err(err).context("failed to get file content")?,
        };

        let current_session = gb_repository
            .get_or_create_current_session()
            .context("failed to get or create current session")?;

        let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)
            .context("failed to get session reader")?;

        let latest_file_content = match current_session_reader.file(path) {
            Ok(content) => Some(content),
            Err(reader::Error::NotFound) => None,
            Err(err) => Err(err).context("failed to get file content")?,
        };

        let current_deltas = Self::get_current_deltas(&gb_repository, path)
            .with_context(|| "failed to get current deltas")?;

        let mut text_doc = deltas::Document::new(
            latest_file_content.as_ref(),
            current_deltas.unwrap_or_default(),
        )?;

        let new_delta = text_doc
            .update(current_wd_file_content.as_ref())
            .context("failed to calculate new deltas")?;

        if new_delta.is_none() {
            tracing::debug!(%project_id, path = %path.display(), "no new deltas, ignoring");
            return Ok(vec![]);
        }
        let new_delta = new_delta.as_ref().unwrap();

        let deltas = text_doc.get_deltas();

        let writer = deltas::Writer::new(&gb_repository);
        writer
            .write(path, &deltas)
            .with_context(|| "failed to write deltas")?;

        if let Some(reader::Content::UTF8(text)) = current_wd_file_content {
            writer.write_wd_file(path, &text)
        } else {
            writer.write_wd_file(path, "")
        }?;

        Ok(vec![
            events::Event::SessionFile((
                *project_id,
                current_session.id,
                path.to_path_buf(),
                latest_file_content,
            )),
            events::Event::Session(*project_id, current_session.clone()),
            events::Event::SessionDelta((
                *project_id,
                current_session.id,
                path.to_path_buf(),
                new_delta.clone(),
            )),
        ])
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use once_cell::sync::Lazy;

    use crate::{
        deltas, sessions,
        test_utils::{self, Case, Suite},
        virtual_branches::{self, branch},
    };

    use self::branch::BranchId;

    use super::*;

    static TEST_TARGET_INDEX: Lazy<AtomicUsize> = Lazy::new(|| AtomicUsize::new(0));

    fn test_target() -> virtual_branches::target::Target {
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

    fn test_branch() -> virtual_branches::branch::Branch {
        TEST_INDEX.fetch_add(1, Ordering::Relaxed);

        virtual_branches::branch::Branch {
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
            ownership: branch::Ownership::default(),
            order: TEST_INDEX.load(Ordering::Relaxed),
        }
    }

    #[test]
    fn test_register_existing_commited_file() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case_with_files(HashMap::from([(path::PathBuf::from("test.txt"), "test")]));
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test2")?;
        listener.handle("test.txt", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(
            deltas[0].operations[0],
            deltas::Operation::Insert((4, "2".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test2"
        );

        Ok(())
    }

    #[test]
    fn test_register_must_init_current_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.handle("test.txt", &project.id)?;

        assert!(gb_repository.get_current_session()?.is_some());

        Ok(())
    }

    #[test]
    fn test_register_must_not_override_current_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.handle("test.txt", &project.id)?;
        let session1 = gb_repository.get_current_session()?.unwrap();

        std::fs::write(project.path.join("test.txt"), "test2")?;
        listener.handle("test.txt", &project.id)?;
        let session2 = gb_repository.get_current_session()?.unwrap();

        assert_eq!(session1.id, session2.id);

        Ok(())
    }

    #[test]
    fn test_register_binfile() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(
            project.path.join("test.bin"),
            [0, 159, 146, 150, 159, 146, 150],
        )?;

        listener.handle("test.bin", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
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
    fn test_register_empty_new_file() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "")?;

        listener.handle("test.txt", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
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
    fn test_register_new_file() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test")?;

        listener.handle("test.txt", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(
            deltas[0].operations[0],
            deltas::Operation::Insert((0, "test".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test"
        );

        Ok(())
    }

    #[test]
    fn test_register_new_file_twice() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.handle("test.txt", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(
            deltas[0].operations[0],
            deltas::Operation::Insert((0, "test".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test"
        );

        std::fs::write(project.path.join("test.txt"), "test2")?;
        listener.handle("test.txt", &project.id)?;

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
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test2"
        );

        Ok(())
    }

    #[test]
    fn test_register_file_deleted() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.handle("test.txt", &project.id)?;

        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas = deltas_reader.read_file("test.txt")?.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].operations.len(), 1);
        assert_eq!(
            deltas[0].operations[0],
            deltas::Operation::Insert((0, "test".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test"
        );

        std::fs::remove_file(project.path.join("test.txt"))?;
        listener.handle("test.txt", &project.id)?;

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
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        let size = 10;
        let relative_file_path = std::path::Path::new("one/two/test.txt");
        for i in 1..=size {
            std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
            // create a session with a single file change and flush it
            std::fs::write(
                std::path::Path::new(&project.path).join(relative_file_path),
                i.to_string(),
            )?;

            test_utils::commit_all(&project_repository.git_repository);
            listener.handle(relative_file_path, &project.id)?;
            assert!(gb_repository.flush(&project_repository, None)?.is_some());
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
            let mut operations: Vec<deltas::Operation> = vec![];
            for session in &mut *sessions_slice {
                let session_reader = sessions::Reader::open(&gb_repository, session).unwrap();
                let deltas_reader = deltas::Reader::new(&session_reader);
                let deltas_by_filepath = deltas_reader.read(&None).unwrap();
                for deltas in deltas_by_filepath.values() {
                    for delta in deltas {
                        delta.operations.iter().for_each(|operation| {
                            operations.push(operation.clone());
                        });
                    }
                }
            }

            let reader =
                sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap()).unwrap();
            let files = reader.files(&None).unwrap();

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
    fn test_flow_no_commits() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        let size = 10;
        let relative_file_path = std::path::Path::new("one/two/test.txt");
        for i in 1..=size {
            std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
            // create a session with a single file change and flush it
            std::fs::write(
                std::path::Path::new(&project.path).join(relative_file_path),
                i.to_string(),
            )?;

            listener.handle(relative_file_path, &project.id)?;
            assert!(gb_repository.flush(&project_repository, None)?.is_some());
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
            let mut operations: Vec<deltas::Operation> = vec![];
            for session in &mut *sessions_slice {
                let session_reader = sessions::Reader::open(&gb_repository, session).unwrap();
                let deltas_reader = deltas::Reader::new(&session_reader);
                let deltas_by_filepath = deltas_reader.read(&None).unwrap();
                for deltas in deltas_by_filepath.values() {
                    for delta in deltas {
                        delta.operations.iter().for_each(|operation| {
                            operations.push(operation.clone());
                        });
                    }
                }
            }

            let reader =
                sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap()).unwrap();
            let files = reader.files(&None).unwrap();

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
    fn test_flow_signle_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::from(&suite.local_app_data);

        let size = 10_i32;
        let relative_file_path = std::path::Path::new("one/two/test.txt");
        for i in 1_i32..=size {
            std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
            // create a session with a single file change and flush it
            std::fs::write(
                std::path::Path::new(&project.path).join(relative_file_path),
                i.to_string(),
            )?;

            listener.handle(relative_file_path, &project.id)?;
        }

        // collect all operations from sessions in the reverse order
        let mut operations: Vec<deltas::Operation> = vec![];
        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session).unwrap();
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas_by_filepath = deltas_reader.read(&None).unwrap();
        for deltas in deltas_by_filepath.values() {
            for delta in deltas {
                delta.operations.iter().for_each(|operation| {
                    operations.push(operation.clone());
                });
            }
        }

        let reader = sessions::Reader::open(&gb_repository, &session).unwrap();
        let files = reader.files(&None).unwrap();

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
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = suite.new_case_with_files(HashMap::from([(
            path::PathBuf::from("test.txt"),
            "hello world",
        )]));
        let listener = Handler::from(&suite.local_app_data);

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repository);
        let target_writer = virtual_branches::target::Writer::new(&gb_repository);
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&vbranch1)?;
        target_writer.write(&vbranch1.id, &vbranch1_target)?;

        std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
        listener.handle("test.txt", &project.id)?;

        let flushed_session = gb_repository.flush(&project_repository, None).unwrap();

        // create a new session
        let session = gb_repository.get_or_create_current_session().unwrap();
        assert_ne!(session.id, flushed_session.unwrap().id);

        // ensure that the virtual branch is still there and selected
        let session_reader = sessions::Reader::open(&gb_repository, &session).unwrap();

        let branches = virtual_branches::Iterator::new(&session_reader)
            .unwrap()
            .collect::<Result<Vec<virtual_branches::Branch>, crate::reader::Error>>()
            .unwrap()
            .into_iter()
            .collect::<Vec<virtual_branches::Branch>>();
        assert_eq!(branches.len(), 2);
        let branch_ids = branches.iter().map(|b| b.id).collect::<Vec<_>>();
        assert!(branch_ids.contains(&vbranch0.id));
        assert!(branch_ids.contains(&vbranch1.id));

        let target_reader = virtual_branches::target::Reader::new(&session_reader);
        assert_eq!(target_reader.read_default().unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

        Ok(())
    }

    #[test]
    fn should_restore_branches_targets_state_from_head_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = suite.new_case_with_files(HashMap::from([(
            path::PathBuf::from("test.txt"),
            "hello world",
        )]));
        let listener = Handler::from(&suite.local_app_data);

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repository);
        let target_writer = virtual_branches::target::Writer::new(&gb_repository);
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&vbranch1)?;
        target_writer.write(&vbranch1.id, &vbranch1_target)?;

        std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
        listener.handle("test.txt", &project.id).unwrap();

        let flushed_session = gb_repository.flush(&project_repository, None).unwrap();

        // hard delete branches state from disk
        std::fs::remove_dir_all(gb_repository.root()).unwrap();

        // create a new session
        let session = gb_repository.get_or_create_current_session().unwrap();
        assert_ne!(session.id, flushed_session.unwrap().id);

        // ensure that the virtual branch is still there and selected
        let session_reader = sessions::Reader::open(&gb_repository, &session).unwrap();

        let branches = virtual_branches::Iterator::new(&session_reader)
            .unwrap()
            .collect::<Result<Vec<virtual_branches::Branch>, crate::reader::Error>>()
            .unwrap()
            .into_iter()
            .collect::<Vec<virtual_branches::Branch>>();
        assert_eq!(branches.len(), 2);
        let branch_ids = branches.iter().map(|b| b.id).collect::<Vec<_>>();
        assert!(branch_ids.contains(&vbranch0.id));
        assert!(branch_ids.contains(&vbranch1.id));

        let target_reader = virtual_branches::target::Reader::new(&session_reader);
        assert_eq!(target_reader.read_default().unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

        Ok(())
    }
}
