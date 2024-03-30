mod database {
    use std::path;

    use gitbutler_core::{
        deltas::{operations, Database, Delta},
        projects::ProjectId,
        sessions::SessionId,
    };

    use crate::shared::test_database;

    #[test]
    fn insert_query() -> anyhow::Result<()> {
        let (db, _tmp) = test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path = path::PathBuf::from("file_path");
        let delta1 = Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let deltas = vec![delta1.clone()];

        database.insert(&project_id, &session_id, &file_path, &deltas)?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id, &None)?,
            vec![(file_path.display().to_string(), vec![delta1])]
                .into_iter()
                .collect()
        );

        Ok(())
    }

    #[test]
    fn insert_update() -> anyhow::Result<()> {
        let (db, _tmp) = test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path = path::PathBuf::from("file_path");
        let delta1 = Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let delta2 = Delta {
            timestamp_ms: 0,
            operations: vec![operations::Operation::Insert((
                0,
                "updated_text".to_string(),
            ))],
        };

        database.insert(&project_id, &session_id, &file_path, &vec![delta1])?;
        database.insert(&project_id, &session_id, &file_path, &vec![delta2.clone()])?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id, &None)?,
            vec![(file_path.display().to_string(), vec![delta2])]
                .into_iter()
                .collect()
        );

        Ok(())
    }

    #[test]
    fn aggregate_deltas_by_file() -> anyhow::Result<()> {
        let (db, _tmp) = test_database();
        let database = Database::new(db);

        let project_id = ProjectId::generate();
        let session_id = SessionId::generate();
        let file_path1 = path::PathBuf::from("file_path1");
        let file_path2 = path::PathBuf::from("file_path2");
        let delta1 = Delta {
            timestamp_ms: 1,
            operations: vec![operations::Operation::Insert((0, "text".to_string()))],
        };
        let delta2 = Delta {
            timestamp_ms: 2,
            operations: vec![operations::Operation::Insert((
                0,
                "updated_text".to_string(),
            ))],
        };

        database.insert(&project_id, &session_id, &file_path1, &vec![delta1.clone()])?;
        database.insert(&project_id, &session_id, &file_path2, &vec![delta1.clone()])?;
        database.insert(&project_id, &session_id, &file_path2, &vec![delta2.clone()])?;

        assert_eq!(
            database.list_by_project_id_session_id(&project_id, &session_id, &None)?,
            vec![
                (file_path1.display().to_string(), vec![delta1.clone()]),
                (file_path2.display().to_string(), vec![delta1, delta2])
            ]
            .into_iter()
            .collect()
        );

        Ok(())
    }
}

mod document;
mod operations;

mod writer {
    use std::vec;

    use gitbutler_core::{deltas, deltas::operations::Operation, sessions};

    use crate::shared::{Case, Suite};

    #[test]
    fn write_no_vbranches() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { gb_repository, .. } = &suite.new_case();

        let deltas_writer = deltas::Writer::new(gb_repository)?;

        let session = gb_repository.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(gb_repository, &session)?;
        let deltas_reader = gitbutler_core::deltas::Reader::new(&session_reader);

        let path = "test.txt";
        let deltas = vec![
            gitbutler_core::deltas::Delta {
                operations: vec![Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            gitbutler_core::deltas::Delta {
                operations: vec![Operation::Insert((5, " world".to_string()))],
                timestamp_ms: 0,
            },
        ];

        deltas_writer.write(path, &deltas).unwrap();

        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas));
        assert_eq!(deltas_reader.read_file("not found").unwrap(), None);

        Ok(())
    }
}
