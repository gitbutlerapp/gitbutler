mod handler {
    use crate::init_opts_bare;

    fn test_remote_repository() -> anyhow::Result<git2::Repository> {
        let path = tempfile::tempdir()?.path().to_str().unwrap().to_string();
        let repo_a = git2::Repository::init_opts(path, &init_opts_bare())?;

        Ok(repo_a)
    }

    mod calculate_delta_handler {
        use anyhow::Result;
        use std::path::{Path, PathBuf};
        use std::{
            collections::HashMap,
            sync::atomic::{AtomicUsize, Ordering},
        };

        use once_cell::sync::Lazy;

        use crate::{commit_all, Case, Suite};
        use gitbutler_app::watcher::handlers::calculate_deltas_handler::Handler;
        use gitbutler_app::{
            deltas::{self, operations::Operation},
            reader, sessions,
            virtual_branches::{self, branch},
        };

        use self::branch::BranchId;

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

        fn test_branch() -> branch::Branch {
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
        fn test_register_existing_commited_file() -> Result<()> {
            let suite = Suite::default();
            let Case {
                gb_repository,
                project,
                ..
            } = suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
            let listener = Handler::from_path(&suite.local_app_data);

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
                    files[Path::new("test.txt")],
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
            let listener = Handler::from_path(&suite.local_app_data);

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
                    sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap())
                        .unwrap();
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
            let listener = Handler::from_path(&suite.local_app_data);

            let size = 10;
            let relative_file_path = Path::new("one/two/test.txt");
            for i in 1..=size {
                std::fs::create_dir_all(Path::new(&project.path).join("one/two"))?;
                // create a session with a single file change and flush it
                std::fs::write(
                    Path::new(&project.path).join(relative_file_path),
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
                    sessions::Reader::open(&gb_repository, sessions_slice.first().unwrap())
                        .unwrap();
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
            let listener = Handler::from_path(&suite.local_app_data);

            let size = 10_i32;
            let relative_file_path = Path::new("one/two/test.txt");
            for i in 1_i32..=size {
                std::fs::create_dir_all(Path::new(&project.path).join("one/two"))?;
                // create a session with a single file change and flush it
                std::fs::write(
                    Path::new(&project.path).join(relative_file_path),
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
            } = suite
                .new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "hello world")]));
            let listener = Handler::from_path(&suite.local_app_data);

            let branch_writer = branch::Writer::new(&gb_repository, project.gb_dir())?;
            let target_writer =
                virtual_branches::target::Writer::new(&gb_repository, project.gb_dir())?;
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
                .collect::<Result<Vec<virtual_branches::Branch>, gitbutler_app::reader::Error>>()
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
            } = suite
                .new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "hello world")]));
            let listener = Handler::from_path(&suite.local_app_data);

            let branch_writer = branch::Writer::new(&gb_repository, project.gb_dir())?;
            let target_writer =
                virtual_branches::target::Writer::new(&gb_repository, project.gb_dir())?;
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
                .collect::<Result<Vec<virtual_branches::Branch>, gitbutler_app::reader::Error>>()
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
                let listener = Handler::from_path(&suite.local_app_data);

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
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
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
                listener.handle("one/two/test2.txt", &project.id).unwrap();

                let flushed_session = gb_repository
                    .flush(&project_repository, None)
                    .unwrap()
                    .unwrap();
                {
                    // after flush, it should be flushed into the commit next to the previous one
                    let session_commit = gb_repository
                        .git_repository()
                        .find_commit(flushed_session.hash.unwrap())
                        .unwrap();
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
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
                let suite = Suite::default();
                let Case {
                    gb_repository,
                    project,
                    project_repository,
                    ..
                } = suite.new_case();
                let listener = Handler::from_path(&suite.local_app_data);

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
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
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
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
                    .unwrap();
                    assert!(commit_reader
                        .list_files(Path::new("wd"))
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
                let listener = Handler::from_path(&suite.local_app_data);

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
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
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
                    let commit_reader = reader::Reader::from_commit(
                        gb_repository.git_repository(),
                        &session_commit,
                    )
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
    }

    mod fetch_gitbutler_data {
        use std::time::SystemTime;

        use gitbutler_app::projects;
        use pretty_assertions::assert_eq;

        use crate::watcher::handler::test_remote_repository;
        use crate::{Case, Suite};
        use gitbutler_app::watcher::handlers::fetch_gitbutler_data::InnerHandler;

        #[tokio::test]
        async fn test_fetch_success() -> anyhow::Result<()> {
            let suite = Suite::default();
            let Case { project, .. } = suite.new_case();

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

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            let listener = InnerHandler {
                local_data_dir: suite.local_app_data,
                projects: suite.projects,
                users: suite.users,
            };

            listener
                .handle(&project.id, &SystemTime::now())
                .await
                .unwrap();

            Ok(())
        }

        #[tokio::test]
        async fn test_fetch_fail_no_sync() {
            let suite = Suite::default();
            let Case { project, .. } = suite.new_case();

            let listener = InnerHandler {
                local_data_dir: suite.local_app_data,
                projects: suite.projects,
                users: suite.users,
            };

            let res = listener.handle(&project.id, &SystemTime::now()).await;

            assert_eq!(&res.unwrap_err().to_string(), "sync disabled");
        }
    }

    mod git_file_change {
        use anyhow::Result;
        use std::fs;

        use gitbutler_app::projects;
        use pretty_assertions::assert_eq;

        use crate::{Case, Suite};
        use gitbutler_app::watcher::handlers::git_file_change::Handler;
        use gitbutler_app::watcher::{handlers, Event};

        #[test]
        fn test_flush_session() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                ..
            } = suite.new_case();

            assert!(gb_repository.get_current_session()?.is_none());
            create_new_session_via_new_file(&project, &suite);
            assert!(gb_repository.get_current_session()?.is_some());

            let listener = Handler::new(suite.local_app_data, suite.projects, suite.users);

            let flush_file_path = project.path.join(".git/GB_FLUSH");
            fs::write(flush_file_path.as_path(), "")?;

            let result = listener.handle("GB_FLUSH", &project.id)?;

            assert_eq!(result.len(), 1);
            assert!(matches!(result[0], Event::Flush(_, _)));

            assert!(!flush_file_path.exists(), "flush file deleted");

            Ok(())
        }

        #[test]
        fn test_do_not_flush_session_if_file_is_missing() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                ..
            } = suite.new_case();

            assert!(gb_repository.get_current_session()?.is_none());
            create_new_session_via_new_file(&project, &suite);
            assert!(gb_repository.get_current_session()?.is_some());

            let listener = Handler::new(suite.local_app_data, suite.projects, suite.users);

            let result = listener.handle("GB_FLUSH", &project.id)?;

            assert_eq!(result.len(), 0);

            Ok(())
        }

        fn create_new_session_via_new_file(project: &projects::Project, suite: &Suite) {
            fs::write(project.path.join("test.txt"), "test").unwrap();

            let file_change_listener =
                handlers::calculate_deltas_handler::Handler::from_path(&suite.local_app_data);
            file_change_listener
                .handle("test.txt", &project.id)
                .unwrap();
        }

        #[test]
        fn test_flush_deletes_flush_file_without_session_to_flush() -> Result<()> {
            let suite = Suite::default();
            let Case { project, .. } = suite.new_case();

            let listener = Handler::new(suite.local_app_data, suite.projects, suite.users);

            let flush_file_path = project.path.join(".git/GB_FLUSH");
            fs::write(flush_file_path.as_path(), "")?;

            let result = listener.handle("GB_FLUSH", &project.id)?;

            assert_eq!(result.len(), 0);

            assert!(!flush_file_path.exists(), "flush file deleted");

            Ok(())
        }
    }

    mod push_project_to_gitbutler {
        use anyhow::Result;
        use gitbutler_app::{git, projects};
        use std::collections::HashMap;
        use std::path::PathBuf;

        use crate::virtual_branches::set_test_target;
        use crate::watcher::handler::test_remote_repository;
        use crate::{Case, Suite};
        use gitbutler_app::project_repository::LogUntil;
        use gitbutler_app::watcher::handlers::push_project_to_gitbutler::HandlerInner;

        fn log_walk(repo: &git2::Repository, head: git::Oid) -> Vec<git::Oid> {
            let mut walker = repo.revwalk().unwrap();
            walker.push(head.into()).unwrap();
            walker.map(|oid| oid.unwrap().into()).collect::<Vec<_>>()
        }

        #[tokio::test]
        async fn test_push_error() -> Result<()> {
            let suite = Suite::default();
            let Case { project, .. } = suite.new_case();

            let api_project = projects::ApiProject {
                name: "test-sync".to_string(),
                description: None,
                repository_id: "123".to_string(),
                git_url: String::new(),
                code_git_url: Some(String::new()),
                created_at: 0_i32.to_string(),
                updated_at: 0_i32.to_string(),
                sync: true,
            };

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            let listener = HandlerInner {
                local_data_dir: suite.local_app_data,
                project_store: suite.projects,
                users: suite.users,
                batch_size: 100,
            };

            let res = listener.handle(&project.id).await;

            res.unwrap_err();

            Ok(())
        }

        #[tokio::test]
        async fn test_push_simple() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                project_repository,
                ..
            } = suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

            suite.sign_in();

            set_test_target(&gb_repository, &project_repository).unwrap();

            let target_id = gb_repository.default_target().unwrap().unwrap().sha;

            let reference = project_repository.l(target_id, LogUntil::End).unwrap();

            let cloud_code = test_remote_repository()?;

            let api_project = projects::ApiProject {
                name: "test-sync".to_string(),
                description: None,
                repository_id: "123".to_string(),
                git_url: String::new(),
                code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
                created_at: 0_i32.to_string(),
                updated_at: 0_i32.to_string(),
                sync: true,
            };

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            cloud_code.find_commit(target_id.into()).unwrap_err();

            {
                let listener = HandlerInner {
                    local_data_dir: suite.local_app_data,
                    project_store: suite.projects.clone(),
                    users: suite.users,
                    batch_size: 10,
                };

                let res = listener.handle(&project.id).await.unwrap();
                assert!(res.is_empty());
            }

            cloud_code.find_commit(target_id.into()).unwrap();

            let pushed = log_walk(&cloud_code, target_id);
            assert_eq!(reference.len(), pushed.len());
            assert_eq!(reference, pushed);

            assert_eq!(
                suite
                    .projects
                    .get(&project.id)
                    .unwrap()
                    .gitbutler_code_push_state
                    .unwrap()
                    .id,
                target_id
            );

            Ok(())
        }

        #[tokio::test]
        async fn test_push_remote_ref() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                project_repository,
                ..
            } = suite.new_case();

            suite.sign_in();

            set_test_target(&gb_repository, &project_repository).unwrap();

            let cloud_code: git::Repository = test_remote_repository()?.into();

            let remote_repo: git::Repository = test_remote_repository()?.into();

            let last_commit = create_initial_commit(&remote_repo);

            remote_repo
                .reference(
                    &git::Refname::Local(git::LocalRefname::new("refs/heads/testbranch", None)),
                    last_commit,
                    false,
                    "",
                )
                .unwrap();

            let mut remote = project_repository
                .git_repository
                .remote("tr", &remote_repo.path().to_str().unwrap().parse().unwrap())
                .unwrap();

            remote
                .fetch(&["+refs/heads/*:refs/remotes/tr/*"], None)
                .unwrap();

            project_repository
                .git_repository
                .find_commit(last_commit)
                .unwrap();

            let api_project = projects::ApiProject {
                name: "test-sync".to_string(),
                description: None,
                repository_id: "123".to_string(),
                git_url: String::new(),
                code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
                created_at: 0_i32.to_string(),
                updated_at: 0_i32.to_string(),
                sync: true,
            };

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            {
                let listener = HandlerInner {
                    local_data_dir: suite.local_app_data,
                    project_store: suite.projects.clone(),
                    users: suite.users,
                    batch_size: 10,
                };

                listener.handle(&project.id).await.unwrap();
            }

            cloud_code.find_commit(last_commit).unwrap();

            Ok(())
        }

        fn create_initial_commit(repo: &git::Repository) -> git::Oid {
            let signature = git::Signature::now("test", "test@email.com").unwrap();

            let mut index = repo.index().unwrap();
            let oid = index.write_tree().unwrap();

            repo.commit(
                None,
                &signature,
                &signature,
                "initial commit",
                &repo.find_tree(oid).unwrap(),
                &[],
            )
            .unwrap()
        }

        fn create_test_commits(repo: &git::Repository, commits: usize) -> git::Oid {
            let signature = git::Signature::now("test", "test@email.com").unwrap();

            let mut last = None;

            for i in 0..commits {
                let mut index = repo.index().unwrap();
                let oid = index.write_tree().unwrap();
                let head = repo.head().unwrap();

                last = Some(
                    repo.commit(
                        Some(&head.name().unwrap()),
                        &signature,
                        &signature,
                        format!("commit {i}").as_str(),
                        &repo.find_tree(oid).unwrap(),
                        &[&repo
                            .find_commit(repo.refname_to_id("HEAD").unwrap())
                            .unwrap()],
                    )
                    .unwrap(),
                );
            }

            last.unwrap()
        }

        #[tokio::test]
        async fn test_push_batches() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                project_repository,
                ..
            } = suite.new_case();

            suite.sign_in();

            {
                let head: git::Oid = project_repository
                    .get_head()
                    .unwrap()
                    .peel_to_commit()
                    .unwrap()
                    .id();

                let reference = project_repository.l(head, LogUntil::End).unwrap();
                assert_eq!(reference.len(), 2);

                let head = create_test_commits(&project_repository.git_repository, 10);

                let reference = project_repository.l(head, LogUntil::End).unwrap();
                assert_eq!(reference.len(), 12);
            }

            set_test_target(&gb_repository, &project_repository).unwrap();

            let target_id = gb_repository.default_target().unwrap().unwrap().sha;

            let reference = project_repository.l(target_id, LogUntil::End).unwrap();

            let cloud_code = test_remote_repository()?;

            let api_project = projects::ApiProject {
                name: "test-sync".to_string(),
                description: None,
                repository_id: "123".to_string(),
                git_url: String::new(),
                code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
                created_at: 0_i32.to_string(),
                updated_at: 0_i32.to_string(),
                sync: true,
            };

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            {
                let listener = HandlerInner {
                    local_data_dir: suite.local_app_data.clone(),
                    project_store: suite.projects.clone(),
                    users: suite.users.clone(),
                    batch_size: 2,
                };

                listener.handle(&project.id).await.unwrap();
            }

            cloud_code.find_commit(target_id.into()).unwrap();

            let pushed = log_walk(&cloud_code, target_id);
            assert_eq!(reference.len(), pushed.len());
            assert_eq!(reference, pushed);

            assert_eq!(
                suite
                    .projects
                    .get(&project.id)
                    .unwrap()
                    .gitbutler_code_push_state
                    .unwrap()
                    .id,
                target_id
            );

            Ok(())
        }

        #[tokio::test]
        async fn test_push_again_no_change() -> Result<()> {
            let suite = Suite::default();
            let Case {
                project,
                gb_repository,
                project_repository,
                ..
            } = suite.new_case_with_files(HashMap::from([(PathBuf::from("test.txt"), "test")]));

            suite.sign_in();

            set_test_target(&gb_repository, &project_repository).unwrap();

            let target_id = gb_repository.default_target().unwrap().unwrap().sha;

            let reference = project_repository.l(target_id, LogUntil::End).unwrap();

            let cloud_code = test_remote_repository()?;

            let api_project = projects::ApiProject {
                name: "test-sync".to_string(),
                description: None,
                repository_id: "123".to_string(),
                git_url: String::new(),
                code_git_url: Some(cloud_code.path().to_str().unwrap().to_string()),
                created_at: 0_i32.to_string(),
                updated_at: 0_i32.to_string(),
                sync: true,
            };

            suite
                .projects
                .update(&projects::UpdateRequest {
                    id: project.id,
                    api: Some(api_project.clone()),
                    ..Default::default()
                })
                .await?;

            cloud_code.find_commit(target_id.into()).unwrap_err();

            {
                let listener = HandlerInner {
                    local_data_dir: suite.local_app_data,
                    project_store: suite.projects.clone(),
                    users: suite.users,
                    batch_size: 10,
                };

                let res = listener.handle(&project.id).await.unwrap();
                assert!(res.is_empty());
            }

            cloud_code.find_commit(target_id.into()).unwrap();

            let pushed = log_walk(&cloud_code, target_id);
            assert_eq!(reference.len(), pushed.len());
            assert_eq!(reference, pushed);

            assert_eq!(
                suite
                    .projects
                    .get(&project.id)
                    .unwrap()
                    .gitbutler_code_push_state
                    .unwrap()
                    .id,
                target_id
            );

            Ok(())
        }
    }
}
