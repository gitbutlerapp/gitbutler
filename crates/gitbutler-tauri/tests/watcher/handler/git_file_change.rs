use std::fs;

use anyhow::Result;
use gitbutler_core::projects;
use gitbutler_tauri::watcher::{Handler, PrivateEvent};
use pretty_assertions::assert_eq;

use gitbutler_testsupport::{Case, Suite};

#[test]
fn flush_session() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        ..
    } = &suite.new_case();

    assert!(gb_repository.get_current_session()?.is_none());
    create_new_session_via_new_file(project, &suite);
    assert!(gb_repository.get_current_session()?.is_some());

    let flush_file_path = project.path.join(".git/GB_FLUSH");
    fs::write(flush_file_path.as_path(), "")?;

    let result = Handler::git_file_change_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        "GB_FLUSH",
        project.id,
    )?;

    assert_eq!(result.len(), 1);
    assert!(matches!(result[0], PrivateEvent::Flush(_, _)));
    assert!(!flush_file_path.exists(), "flush file deleted");

    Ok(())
}

#[test]
fn do_not_flush_session_if_file_is_missing() -> Result<()> {
    let suite = Suite::default();
    let Case {
        project,
        gb_repository,
        ..
    } = &suite.new_case();

    assert!(gb_repository.get_current_session()?.is_none());
    create_new_session_via_new_file(project, &suite);
    assert!(gb_repository.get_current_session()?.is_some());

    let result = Handler::git_file_change_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        "GB_FLUSH",
        project.id,
    )?;

    assert_eq!(result.len(), 0);
    Ok(())
}

fn create_new_session_via_new_file(project: &projects::Project, suite: &Suite) {
    fs::write(project.path.join("test.txt"), "test").unwrap();

    let file_change_listener = super::calculate_delta::State::from_path(suite.local_app_data());
    file_change_listener
        .calculate_delta("test.txt", project.id)
        .unwrap();
}

#[test]
fn flush_deletes_flush_file_without_session_to_flush() -> Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let flush_file_path = project.path.join(".git/GB_FLUSH");
    fs::write(flush_file_path.as_path(), "")?;

    let result = Handler::git_file_change_pure(
        suite.local_app_data(),
        &suite.projects,
        &suite.users,
        "GB_FLUSH",
        project.id,
    )?;

    assert_eq!(result.len(), 0);
    assert!(!flush_file_path.exists(), "flush file deleted");

    Ok(())
}
