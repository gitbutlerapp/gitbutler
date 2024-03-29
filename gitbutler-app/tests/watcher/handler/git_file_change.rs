use anyhow::Result;
use std::fs;

use gitbutler_app::projects;
use pretty_assertions::assert_eq;

use crate::{Case, Suite};
use gitbutler_app::watcher::handlers::git_file_change::Handler;
use gitbutler_app::watcher::{handlers, Event};

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

    let listener = Handler::new(
        suite.local_app_data().into(),
        suite.projects.clone(),
        suite.users.clone(),
    );

    let flush_file_path = project.path.join(".git/GB_FLUSH");
    fs::write(flush_file_path.as_path(), "")?;

    let result = listener.handle("GB_FLUSH", &project.id)?;

    assert_eq!(result.len(), 1);
    assert!(matches!(result[0], Event::Flush(_, _)));

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

    let listener = Handler::new(
        suite.local_app_data().into(),
        suite.projects.clone(),
        suite.users.clone(),
    );

    let result = listener.handle("GB_FLUSH", &project.id)?;

    assert_eq!(result.len(), 0);

    Ok(())
}

fn create_new_session_via_new_file(project: &projects::Project, suite: &Suite) {
    fs::write(project.path.join("test.txt"), "test").unwrap();

    let file_change_listener =
        handlers::calculate_deltas_handler::Handler::from_path(suite.local_app_data());
    file_change_listener
        .handle("test.txt", &project.id)
        .unwrap();
}

#[test]
fn flush_deletes_flush_file_without_session_to_flush() -> Result<()> {
    let suite = Suite::default();
    let Case { project, .. } = &suite.new_case();

    let listener = Handler::new(
        suite.local_app_data().into(),
        suite.projects.clone(),
        suite.users.clone(),
    );

    let flush_file_path = project.path.join(".git/GB_FLUSH");
    fs::write(flush_file_path.as_path(), "")?;

    let result = listener.handle("GB_FLUSH", &project.id)?;

    assert_eq!(result.len(), 0);

    assert!(!flush_file_path.exists(), "flush file deleted");

    Ok(())
}
