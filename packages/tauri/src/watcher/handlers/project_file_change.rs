use std::{path, vec};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    deltas, gb_repository, project_repository, projects,
    reader::{self, Reader},
    sessions, users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    local_data_dir: path::PathBuf,
    project_store: projects::Storage,
    user_store: users::Storage,
}

impl From<&path::PathBuf> for Handler {
    fn from(local_data_dir: &path::PathBuf) -> Self {
        Self {
            local_data_dir: local_data_dir.to_path_buf(),
            project_store: projects::Storage::from(local_data_dir),
            user_store: users::Storage::from(local_data_dir),
        }
    }
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("Failed to get local data dir")?;
        let user_store = users::Storage::try_from(value).context("Failed to get user store")?;
        let project_store = projects::Storage::try_from(value)?;
        Ok(Self {
            project_store,
            local_data_dir,
            user_store,
        })
    }
}

impl Handler {
    // Returns Some(file_content) or None if the file is ignored.
    fn get_current_file_content(
        &self,
        project_repository: &project_repository::Repository,
        project_id: &str,
        path: &std::path::Path,
    ) -> Result<Option<String>> {
        if project_repository.is_path_ignored(path)? {
            return Ok(None);
        }

        let reader = project_repository.get_wd_reader();

        let path = path.to_str().unwrap();
        if !reader.exists(path) {
            return Ok(Some(String::new()));
        }

        if reader.size(path)? > 100_000 {
            tracing::warn!(project_id, path, "ignoring large file");
            return Ok(None);
        }

        match reader.read(path)? {
            reader::Content::UTF8(content) => Ok(Some(content)),
            reader::Content::Binary(_) => {
                tracing::warn!(project_id, path, "ignoring non-utf8 file");
                Ok(None)
            }
        }
    }

    // returns deltas for the file that are already part of the current session (if any)
    fn get_current_deltas(
        &self,
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
        project_id: &str,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;

        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            project_id,
            self.project_store.clone(),
            self.user_store.clone(),
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
            if session.meta.branch != project_head.name().map(|s| s.to_string()) {
                gb_repository
                    .flush_session(&project_repository, &session)
                    .context("failed to flush session")?;
            }
        }

        let path = path.as_ref();

        let current_wd_file_content = match self
            .get_current_file_content(&project_repository, project_id, path)
            .context("failed to get current file content")?
        {
            Some(content) => content,
            None => return Ok(vec![]),
        };

        let current_session = gb_repository
            .get_or_create_current_session()
            .context("failed to get or create current session")?;
        let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)
            .context("failed to get session reader")?;

        let latest_file_content = match current_session_reader.file(path) {
            Ok(reader::Content::UTF8(content)) => content,
            Ok(reader::Content::Binary(_)) => {
                tracing::warn!(project_id, path = %path.display(), "ignoring non-utf8 file");
                return Ok(vec![]);
            }
            Err(reader::Error::NotFound) => "".to_string(),
            Err(err) => Err(err).context("failed to get file content")?,
        };

        let current_deltas = self
            .get_current_deltas(&gb_repository, path)
            .with_context(|| "failed to get current deltas")?;

        let mut text_doc = deltas::Document::new(
            Some(&latest_file_content),
            current_deltas.unwrap_or_default(),
        )?;

        let new_delta = text_doc
            .update(&current_wd_file_content)
            .context("failed to calculate new deltas")?;
        if new_delta.is_none() {
            tracing::debug!(project_id, path = %path.display(), "no new deltas, ignoring");
            return Ok(vec![]);
        }
        let new_delta = new_delta.as_ref().unwrap();

        let deltas = text_doc.get_deltas();
        let writer = deltas::Writer::new(&gb_repository);
        writer
            .write(path, &deltas)
            .with_context(|| "failed to write deltas")?;
        writer
            .write_wd_file(path, &current_wd_file_content)
            .with_context(|| "failed to write file")?;

        Ok(vec![
            events::Event::SessionFile((
                project_id.to_string(),
                current_session.id.clone(),
                path.to_path_buf(),
                latest_file_content,
            )),
            events::Event::Session(project_id.to_string(), current_session.clone()),
            events::Event::SessionDelta((
                project_id.to_string(),
                current_session.id.clone(),
                path.to_path_buf(),
                new_delta.clone(),
            )),
        ])
    }
}

#[cfg(test)]
mod test {
    use crate::{
        deltas, gb_repository, project_repository, projects, sessions, test_utils, users,
        virtual_branches::{self, branch},
    };

    use super::*;

    static mut TEST_TARGET_INDEX: usize = 0;

    fn test_target() -> virtual_branches::target::Target {
        virtual_branches::target::Target {
            branch: format!(
                "refs/remotes/remote name {}/branch name {}",
                unsafe { TEST_TARGET_INDEX },
                unsafe { TEST_TARGET_INDEX }
            )
            .parse()
            .unwrap(),
            remote_url: format!("remote url {}", unsafe { TEST_TARGET_INDEX }),
            sha: format!("0123456789abcdef0123456789abcdef0123456{}", unsafe {
                TEST_TARGET_INDEX
            })
            .parse()
            .unwrap(),
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
            notes: format!("branch_notes_{}", unsafe { TEST_INDEX }),
            applied: true,
            upstream: Some(
                format!("refs/remotes/origin/upstream_{}", unsafe { TEST_INDEX })
                    .parse()
                    .unwrap(),
            ),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
            head: format!("0123456789abcdef0123456789abcdef0123456{}", unsafe {
                TEST_INDEX
            })
            .parse()
            .unwrap(),
            tree: format!("0123456789abcdef0123456789abcdef012345{}", unsafe {
                TEST_INDEX + 10
            })
            .parse()
            .unwrap(),
            ownership: branch::Ownership::default(),
            order: unsafe { TEST_INDEX },
        }
    }

    #[test]
    fn test_register_existing_commited_file() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;
        test_utils::commit_all(&repository);

        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        std::fs::write(project_repo.root().join(file_path), "test2")?;
        listener.handle(file_path, &project.id)?;

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;

        listener.handle(file_path, &project.id)?;

        assert!(gb_repo.get_current_session()?.is_some());

        Ok(())
    }

    #[test]
    fn test_register_must_not_override_current_session() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;
        listener.handle(file_path, &project.id)?;

        let session1 = gb_repo.get_current_session()?.unwrap();

        std::fs::write(project_repo.root().join(file_path), "test2")?;
        listener.handle(file_path, &project.id)?;

        let session2 = gb_repo.get_current_session()?.unwrap();
        assert_eq!(session1.id, session2.id);

        Ok(())
    }

    #[test]
    fn test_register_new_file() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;

        listener.handle(file_path, &project.id)?;

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;
        listener.handle(file_path, &project.id)?;

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
        listener.handle(file_path, &project.id)?;

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "test")?;
        listener.handle(file_path, &project.id)?;

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
        listener.handle(file_path, &project.id)?;

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let size = 10;
        let relative_file_path = std::path::Path::new("one/two/test.txt");
        for i in 1..=size {
            std::fs::create_dir_all(std::path::Path::new(&project.path).join("one/two"))?;
            // create a session with a single file change and flush it
            std::fs::write(
                std::path::Path::new(&project.path).join(relative_file_path),
                i.to_string(),
            )?;

            test_utils::commit_all(&repository);
            listener.handle(relative_file_path, &project.id)?;
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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;
        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

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
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "hello world")?;
        test_utils::commit_all(&repository);

        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repo);
        let target_writer = virtual_branches::target::Writer::new(&gb_repo);
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&vbranch1)?;
        target_writer.write(&vbranch1.id, &vbranch1_target)?;

        std::fs::write(project_repo.root().join(file_path), "hello world!").unwrap();
        listener.handle(file_path, &project.id)?;

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

        let target_reader = virtual_branches::target::Reader::new(&session_reader);
        assert_eq!(target_reader.read_default().unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

        Ok(())
    }

    #[test]
    fn should_restore_branches_targets_state_from_head_session() -> Result<()> {
        let repository = test_utils::test_repository();
        let project = projects::Project::try_from(&repository)?;
        let project_repo = project_repository::Repository::open(&project)?;
        let local_data_dir = test_utils::temp_dir();
        let user_store = users::Storage::from(&local_data_dir);
        let project_store = projects::Storage::from(&local_data_dir);
        project_store.add_project(&project)?;

        let file_path = std::path::Path::new("test.txt");
        std::fs::write(project_repo.root().join(file_path), "hello world")?;
        test_utils::commit_all(&repository);

        let gb_repo = gb_repository::Repository::open(
            local_data_dir.clone(),
            &project.id,
            project_store.clone(),
            user_store.clone(),
        )?;
        let listener = Handler::from(&local_data_dir);

        let branch_writer = virtual_branches::branch::Writer::new(&gb_repo);
        let target_writer = virtual_branches::target::Writer::new(&gb_repo);
        let default_target = test_target();
        target_writer.write_default(&default_target)?;
        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        let vbranch1_target = test_target();
        branch_writer.write(&vbranch1)?;
        target_writer.write(&vbranch1.id, &vbranch1_target)?;

        std::fs::write(project_repo.root().join(file_path), "hello world!").unwrap();
        listener.handle(file_path, &project.id).unwrap();

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

        let target_reader = virtual_branches::target::Reader::new(&session_reader);
        assert_eq!(target_reader.read_default().unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch0.id).unwrap(), default_target);
        assert_eq!(target_reader.read(&vbranch1.id).unwrap(), vbranch1_target);

        Ok(())
    }
}
