use std::fs;

use anyhow::Result;
use gitbutler_core::projects;
use gitbutler_tauri::watcher;
use pretty_assertions::assert_eq;

use crate::watcher::handler::support::Fixture;
use gitbutler_testsupport::Case;

#[tokio::test]
async fn flush_session() -> Result<()> {
    let mut fixture = Fixture::default();
    {
        let case = fixture.new_case();
        let Case {
            project,
            gb_repository,
            ..
        } = &case;

        assert!(gb_repository.get_current_session()?.is_none());
        let handler = create_new_session_via_new_file(project, &mut fixture);
        assert!(gb_repository.get_current_session()?.is_some());

        let flush_file_path = project.path.join(".git/GB_FLUSH");
        fs::write(flush_file_path.as_path(), "")?;

        handler.git_file_change("GB_FLUSH", project.id).await?;
        assert!(!flush_file_path.exists(), "flush file deleted");
    }

    let events = fixture.events();
    assert_eq!(events.len(), 4);
    assert!(events[0].name().ends_with("/files"));
    assert!(events[1].name().ends_with("/sessions"));
    assert!(events[2].name().ends_with("/deltas"));
    assert!(events[3].name().ends_with("/sessions"));
    Ok(())
}

#[tokio::test]
async fn do_not_flush_session_if_file_is_missing() -> Result<()> {
    let mut fixture = Fixture::default();
    {
        let Case {
            project,
            gb_repository,
            ..
        } = &fixture.new_case();

        assert!(gb_repository.get_current_session()?.is_none());
        let handler = create_new_session_via_new_file(project, &mut fixture);
        assert!(gb_repository.get_current_session()?.is_some());

        handler.git_file_change("GB_FLUSH", project.id).await?;
    }
    let events = fixture.events();
    assert_eq!(events.len(), 3);
    assert!(events[0].name().ends_with("/files"));
    assert!(events[1].name().ends_with("/sessions"));
    assert!(events[2].name().ends_with("/deltas"));
    Ok(())
}

#[tokio::test]
async fn flush_deletes_flush_file_without_session_to_flush() -> Result<()> {
    let mut fixture = Fixture::default();
    {
        let handler = fixture.new_handler();
        let Case { project, .. } = &fixture.new_case();

        let flush_file_path = project.path.join(".git/GB_FLUSH");
        fs::write(flush_file_path.as_path(), "")?;

        handler.git_file_change("GB_FLUSH", project.id).await?;
        assert!(!flush_file_path.exists(), "flush file deleted");
    }
    assert_eq!(fixture.events().len(), 0);
    Ok(())
}

fn create_new_session_via_new_file(
    project: &projects::Project,
    fixture: &mut Fixture,
) -> watcher::Handler {
    fs::write(project.path.join("test.txt"), "test").unwrap();

    let handler = fixture.new_handler();
    handler.calculate_deltas("test.txt", project.id).unwrap();
    handler
}
