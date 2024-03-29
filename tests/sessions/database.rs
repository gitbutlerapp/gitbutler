use crate::shared::test_database;
use gitbutler::projects::ProjectId;
use gitbutler::sessions::{session, Database, Session, SessionId};

#[test]
fn insert_query() -> anyhow::Result<()> {
    let (db, _tmp) = test_database();
    println!("0");
    let database = Database::new(db);
    println!("1");

    let project_id = ProjectId::generate();
    let session1 = Session {
        id: SessionId::generate(),
        hash: None,
        meta: session::Meta {
            branch: None,
            commit: None,
            start_timestamp_ms: 1,
            last_timestamp_ms: 2,
        },
    };
    let session2 = session::Session {
        id: SessionId::generate(),
        hash: Some("08f23df1b9c2dec3d0c826a3ae745f9b821a1a26".parse().unwrap()),
        meta: session::Meta {
            branch: Some("branch2".to_string()),
            commit: Some("commit2".to_string()),
            start_timestamp_ms: 3,
            last_timestamp_ms: 4,
        },
    };
    let sessions = vec![&session1, &session2];

    database.insert(&project_id, &sessions)?;

    assert_eq!(
        database.list_by_project_id(&project_id, None)?,
        vec![session2.clone(), session1.clone()]
    );
    assert_eq!(database.get_by_id(&session1.id)?.unwrap(), session1);
    assert_eq!(database.get_by_id(&session2.id)?.unwrap(), session2);
    assert_eq!(database.get_by_id(&SessionId::generate())?, None);

    Ok(())
}

#[test]
fn update() -> anyhow::Result<()> {
    let (db, _tmp) = test_database();
    let database = Database::new(db);

    let project_id = ProjectId::generate();
    let session = session::Session {
        id: SessionId::generate(),
        hash: None,
        meta: session::Meta {
            branch: None,
            commit: None,
            start_timestamp_ms: 1,
            last_timestamp_ms: 2,
        },
    };
    let session_updated = session::Session {
        id: session.id,
        hash: Some("08f23df1b9c2dec3d0c826a3ae745f9b821a1a26".parse().unwrap()),
        meta: session::Meta {
            branch: Some("branch2".to_string()),
            commit: Some("commit2".to_string()),
            start_timestamp_ms: 3,
            last_timestamp_ms: 4,
        },
    };
    database.insert(&project_id, &[&session])?;
    database.insert(&project_id, &[&session_updated])?;

    assert_eq!(
        database.list_by_project_id(&project_id, None)?,
        vec![session_updated.clone()]
    );
    assert_eq!(database.get_by_id(&session.id)?.unwrap(), session_updated);

    Ok(())
}
