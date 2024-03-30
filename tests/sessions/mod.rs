mod database;

use anyhow::Result;
use gitbutler::sessions::{self, session::SessionId};

use crate::shared::{Case, Suite};

#[test]
fn should_not_write_session_with_hash() {
    let suite = Suite::default();
    let Case { gb_repository, .. } = &suite.new_case();

    let session = sessions::Session {
        id: SessionId::generate(),
        hash: Some("08f23df1b9c2dec3d0c826a3ae745f9b821a1a26".parse().unwrap()),
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
    };

    assert!(sessions::Writer::new(gb_repository)
        .unwrap()
        .write(&session)
        .is_err());
}

#[test]
fn should_write_full_session() -> Result<()> {
    let suite = Suite::default();
    let Case { gb_repository, .. } = &suite.new_case();

    let session = sessions::Session {
        id: SessionId::generate(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
    };

    sessions::Writer::new(gb_repository)?.write(&session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/id"))?,
        session.id.to_string()
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/commit"))?,
        "commit"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/branch"))?,
        "branch"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/last"))?,
        "1"
    );

    Ok(())
}

#[test]
fn should_write_partial_session() -> Result<()> {
    let suite = Suite::default();
    let Case { gb_repository, .. } = &suite.new_case();

    let session = sessions::Session {
        id: SessionId::generate(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: None,
            commit: None,
        },
    };

    sessions::Writer::new(gb_repository)?.write(&session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/id"))?,
        session.id.to_string()
    );
    assert!(!gb_repository.session_path().join("meta/commit").exists());
    assert!(!gb_repository.session_path().join("meta/branch").exists());
    assert_eq!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repository.session_path().join("meta/last"))?,
        "1"
    );

    Ok(())
}
