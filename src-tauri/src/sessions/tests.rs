use anyhow::Result;

use crate::{
    gb_repository,
    projects::{self, Project},
    sessions, users,
    test_utils,
};

use super::Writer;


#[test]
fn test_should_not_write_session_with_hash() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = Project::try_from(&repository)?;
    let gb_repo_path = test_utils::temp_dir();
    let local_app_data = test_utils::temp_dir();
    let user_store = users::Storage::from(&local_app_data);
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: Some("hash".to_string()),
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
    };

    assert!(Writer::new(&gb_repo).write(&session).is_err());

    Ok(())
}

#[test]
fn test_should_write_full_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = Project::try_from(&repository)?;
    let gb_repo_path = test_utils::temp_dir();
    let local_app_data = test_utils::temp_dir();
    let user_store = users::Storage::from(&local_app_data);
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: Some("branch".to_string()),
            commit: Some("commit".to_string()),
        },
    };

    Writer::new(&gb_repo).write(&session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/id"))?,
        "session_id"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/commit"))?,
        "commit"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/branch"))?,
        "branch"
    );
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/last"))?,
        "1"
    );

    Ok(())
}

#[test]
fn test_should_write_partial_session() -> Result<()> {
    let repository = test_utils::test_repository();
    let project = Project::try_from(&repository)?;
    let gb_repo_path = test_utils::temp_dir();
    let local_app_data = test_utils::temp_dir();
    let user_store = users::Storage::from(&local_app_data);
    let project_store = projects::Storage::from(&local_app_data);
    project_store.add_project(&project)?;
    let gb_repo =
        gb_repository::Repository::open(gb_repo_path, &project.id, project_store, user_store)?;

    let session = sessions::Session {
        id: "session_id".to_string(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: 0,
            last_timestamp_ms: 1,
            branch: None,
            commit: None,
        },
    };

    Writer::new(&gb_repo).write(&session)?;

    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/id"))?,
        "session_id"
    );
    assert!(!gb_repo.session_path().join("meta/commit").exists());
    assert!(!gb_repo.session_path().join("meta/branch").exists());
    assert_eq!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/start"))?,
        "0"
    );
    assert_ne!(
        std::fs::read_to_string(gb_repo.session_path().join("meta/last"))?,
        "1"
    );

    Ok(())
}
