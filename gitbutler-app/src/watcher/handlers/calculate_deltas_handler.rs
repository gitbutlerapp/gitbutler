use std::{path, vec};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    deltas, gb_repository, project_repository,
    projects::{self, ProjectId},
    reader, sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let handler = Self::new(
                app_data_dir,
                projects::Controller::try_from(value)?,
                users::Controller::try_from(value)?,
            );
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

#[cfg(test)]
impl TryFrom<&std::path::PathBuf> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        let app_data_dir = value.clone();
        let handler = Self::new(
            app_data_dir,
            projects::Controller::try_from(value)?,
            users::Controller::try_from(value)?,
        );
        Ok(handler)
    }
}

impl Handler {
    fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
    ) -> Self {
        Self {
            local_data_dir,
            projects,
            users,
        }
    }

    // Returns Some(file_content) or None if the file is ignored.
    fn get_current_file(
        project_repository: &project_repository::Repository,
        path: &std::path::Path,
    ) -> Result<reader::Content, reader::Error> {
        if project_repository.is_path_ignored(path).unwrap_or(false) {
            return Err(reader::Error::NotFound);
        }
        let full_path = project_repository.project().path.join(path);
        if !full_path.exists() {
            return Err(reader::Error::NotFound);
        }
        reader::Content::try_from(&full_path).map_err(Into::into)
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

        let project_repository = project_repository::Repository::open(&project)
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
            if session.meta.branch != project_head.name().map(|n| n.to_string()) {
                gb_repository
                    .flush_session(&project_repository, &session, user.as_ref())
                    .context(format!("failed to flush session {}", session.id))?;
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

        let deltas_reader = deltas::Reader::new(&current_session_reader);
        let current_deltas = deltas_reader
            .read_file(path)
            .context("failed to get file deltas")?;

        let mut text_doc = deltas::Document::new(
            latest_file_content.as_ref(),
            current_deltas.unwrap_or_default(),
        )?;

        let new_delta = text_doc
            .update(current_wd_file_content.as_ref())
            .context("failed to calculate new deltas")?;

        if let Some(new_delta) = new_delta {
            let deltas = text_doc.get_deltas();

            let writer =
                deltas::Writer::new(&gb_repository).context("failed to open deltas writer")?;
            writer
                .write(path, &deltas)
                .context("failed to write deltas")?;

            match &current_wd_file_content {
                Some(reader::Content::UTF8(text)) => writer.write_wd_file(path, text),
                Some(_) => writer.write_wd_file(path, ""),
                None => writer.remove_wd_file(path),
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
        } else {
            tracing::debug!(%project_id, path = %path.display(), "no new deltas, ignoring");
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        path,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use once_cell::sync::Lazy;

    use crate::{
        deltas::{self, operations::Operation},
        sessions,
        tests::{self, Case, Suite},
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
            selected_for_changes: None,
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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
            Operation::Insert((4, "2".to_string())),
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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
            Operation::Insert((0, "test".to_string())),
        );
        assert_eq!(
            std::fs::read_to_string(gb_repository.session_wd_path().join("test.txt"))?,
            "test"
        );

        Ok(())
    }

    #[test]
    fn test_register_no_changes_saved_thgoughout_flushes() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

        // file change, wd and deltas are written
        std::fs::write(project.path.join("test.txt"), "test")?;
        listener.handle("test.txt", &project.id)?;

        // make two more sessions.
        gb_repository.flush(&project_repository, None)?;
        gb_repository.get_or_create_current_session()?;
        gb_repository.flush(&project_repository, None)?;

        // after some sessions, files from the first change are still there.
        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repository, &session)?;
        let files = session_reader.files(None)?;
        assert_eq!(files.len(), 1);

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
            Operation::Insert((0, "test".to_string())),
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
    fn test_register_file_deleted() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

        {
            // write file
            std::fs::write(project.path.join("test.txt"), "test")?;
            listener.handle("test.txt", &project.id)?;
        }

        {
            // current session must have the deltas, but not the file (it didn't exist)
            let session = gb_repository.get_current_session()?.unwrap();
            let session_reader = sessions::Reader::open(&gb_repository, &session)?;
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

        gb_repository.flush(&project_repository, None)?;

        {
            // file should be available in the next session, but not deltas just yet.
            let session = gb_repository.get_or_create_current_session()?;
            let session_reader = sessions::Reader::open(&gb_repository, &session)?;
            let files = session_reader.files(None).unwrap();
            assert_eq!(files.len(), 1);
            assert_eq!(
                files[std::path::Path::new("test.txt")],
                reader::Content::UTF8("test".to_string())
            );

            let deltas_reader = deltas::Reader::new(&session_reader);
            let deltas = deltas_reader.read(None)?;
            assert!(deltas.is_empty());

            // removing the file
            std::fs::remove_file(project.path.join("test.txt"))?;
            listener.handle("test.txt", &project.id)?;

            // deltas are recorded
            let deltas = deltas_reader.read_file("test.txt")?.unwrap();
            assert_eq!(deltas.len(), 1);
            assert_eq!(deltas[0].operations.len(), 1);
            assert_eq!(deltas[0].operations[0], Operation::Delete((0, 4)),);
        }

        gb_repository.flush(&project_repository, None)?;

        {
            // since file was deleted in the previous session, it should not exist in the new one.
            let session = gb_repository.get_or_create_current_session()?;
            let session_reader = sessions::Reader::open(&gb_repository, &session)?;
            let files = session_reader.files(None).unwrap();
            assert!(files.is_empty());
        }

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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

        let size = 10;
        let relative_file_path = std::path::Path::new("one/two/test.txt");
        for i in 1..=size {
            std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
            // create a session with a single file change and flush it
            std::fs::write(
                std::path::Path::new(&project.path).join(relative_file_path),
                i.to_string(),
            )?;

            tests::commit_all(&project_repository.git_repository);
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
            let mut operations: Vec<Operation> = vec![];
            for session in &mut *sessions_slice {
                let session_reader = sessions::Reader::open(&gb_repository, session).unwrap();
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
                sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap()).unwrap();
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
    fn test_flow_no_commits() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            project_repository,
            ..
        } = suite.new_case();
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
            let mut operations: Vec<Operation> = vec![];
            for session in &mut *sessions_slice {
                let session_reader = sessions::Reader::open(&gb_repository, session).unwrap();
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
                sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap()).unwrap();
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
    fn test_flow_signle_session() -> Result<()> {
        let suite = Suite::default();
        let Case {
            gb_repository,
            project,
            ..
        } = suite.new_case();
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

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
        let mut operations: Vec<Operation> = vec![];
        let session = gb_repository.get_current_session()?.unwrap();
        let session_reader = sessions::Reader::open(&gb_repository, &session).unwrap();
        let deltas_reader = deltas::Reader::new(&session_reader);
        let deltas_by_filepath = deltas_reader.read(None).unwrap();
        for deltas in deltas_by_filepath.values() {
            for delta in deltas {
                delta.operations.iter().for_each(|operation| {
                    operations.push(operation.clone());
                });
            }
        }

        let reader = sessions::Reader::open(&gb_repository, &session).unwrap();
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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repository)?;
        let target_writer = virtual_branches::target::Writer::new(&gb_repository)?;
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let mut vbranch0 = test_branch();
        branch_writer.write(&mut vbranch0)?;
        let mut vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&mut vbranch1)?;
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
        let listener = Handler::try_from(&suite.local_app_data).unwrap();

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repository)?;
        let target_writer = virtual_branches::target::Writer::new(&gb_repository)?;
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let mut vbranch0 = test_branch();
        branch_writer.write(&mut vbranch0)?;
        let mut vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&mut vbranch1)?;
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

    mod flush_wd {
        use super::*;

        #[test]
        fn should_add_new_files_to_session_wd() {
            let suite = Suite::default();
            let Case {
                gb_repository,
                project,
                project_repository,
                ..
            } = suite.new_case();
            let listener = Handler::try_from(&suite.local_app_data).unwrap();

            // write a file into session
            std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
            listener.handle("test.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
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
                    commit_reader.list_files(path::Path::new("wd")).unwrap(),
                    vec![path::Path::new("test.txt")]
                );
                assert_eq!(
                    commit_reader.read(path::Path::new("wd/test.txt")).unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
            }

            // write another file into session
            std::fs::create_dir_all(project.path.join("one/two")).unwrap();
            std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
            listener.handle("one/two/test2.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
                .unwrap()
                .unwrap();
            {
                // after flush it should be flushed into the commit next to the previous one
                let session_commit = gb_repository
                    .git_repository()
                    .find_commit(flushed_session.hash.unwrap())
                    .unwrap();
                let commit_reader =
                    reader::Reader::from_commit(gb_repository.git_repository(), &session_commit)
                        .unwrap();
                assert_eq!(
                    commit_reader.list_files(path::Path::new("wd")).unwrap(),
                    vec![
                        path::Path::new("one/two/test2.txt"),
                        path::Path::new("test.txt"),
                    ]
                );
                assert_eq!(
                    commit_reader.read(path::Path::new("wd/test.txt")).unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
                assert_eq!(
                    commit_reader
                        .read(path::Path::new("wd/one/two/test2.txt"))
                        .unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
            }
        }

        #[test]
        fn should_remove_deleted_files_from_session_wd() {
            let suite = Suite::default();
            let Case {
                gb_repository,
                project,
                project_repository,
                ..
            } = suite.new_case();
            let listener = Handler::try_from(&suite.local_app_data).unwrap();

            // write a file into session
            std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
            listener.handle("test.txt", &project.id).unwrap();
            std::fs::create_dir_all(project.path.join("one/two")).unwrap();
            std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
            listener.handle("one/two/test2.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
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
                    commit_reader.list_files(path::Path::new("wd")).unwrap(),
                    vec![
                        path::Path::new("one/two/test2.txt"),
                        path::Path::new("test.txt"),
                    ]
                );
                assert_eq!(
                    commit_reader.read(path::Path::new("wd/test.txt")).unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
                assert_eq!(
                    commit_reader
                        .read(path::Path::new("wd/one/two/test2.txt"))
                        .unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
            }

            // rm the files
            std::fs::remove_file(project.path.join("test.txt")).unwrap();
            listener.handle("test.txt", &project.id).unwrap();
            std::fs::remove_file(project.path.join("one/two/test2.txt")).unwrap();
            listener.handle("one/two/test2.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
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
                    .list_files(path::Path::new("wd"))
                    .unwrap()
                    .is_empty());
            }
        }

        #[test]
        fn should_update_updated_files_in_session_wd() {
            let suite = Suite::default();
            let Case {
                gb_repository,
                project,
                project_repository,
                ..
            } = suite.new_case();
            let listener = Handler::try_from(&suite.local_app_data).unwrap();

            // write a file into session
            std::fs::write(project.path.join("test.txt"), "hello world!").unwrap();
            listener.handle("test.txt", &project.id).unwrap();
            std::fs::create_dir_all(project.path.join("one/two")).unwrap();
            std::fs::write(project.path.join("one/two/test2.txt"), "hello world!").unwrap();
            listener.handle("one/two/test2.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
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
                    commit_reader.list_files(path::Path::new("wd")).unwrap(),
                    vec![
                        path::Path::new("one/two/test2.txt"),
                        path::Path::new("test.txt"),
                    ]
                );
                assert_eq!(
                    commit_reader.read(path::Path::new("wd/test.txt")).unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
                assert_eq!(
                    commit_reader
                        .read(path::Path::new("wd/one/two/test2.txt"))
                        .unwrap(),
                    reader::Content::UTF8("hello world!".to_string())
                );
            }

            // update the file
            std::fs::write(project.path.join("test.txt"), "hello world!2").unwrap();
            listener.handle("test.txt", &project.id).unwrap();

            std::fs::write(project.path.join("one/two/test2.txt"), "hello world!2").unwrap();
            listener.handle("one/two/test2.txt", &project.id).unwrap();

            let flushed_session = gb_repository
                .flush(&project_repository, None)
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
                    commit_reader.list_files(path::Path::new("wd")).unwrap(),
                    vec![
                        path::Path::new("one/two/test2.txt"),
                        path::Path::new("test.txt"),
                    ]
                );
                assert_eq!(
                    commit_reader.read(path::Path::new("wd/test.txt")).unwrap(),
                    reader::Content::UTF8("hello world!2".to_string())
                );
                assert_eq!(
                    commit_reader
                        .read(path::Path::new("wd/one/two/test2.txt"))
                        .unwrap(),
                    reader::Content::UTF8("hello world!2".to_string())
                );
            }
        }
    }
}
