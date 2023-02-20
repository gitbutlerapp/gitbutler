use crate::projects;
use anyhow::Result;
use tempfile::tempdir;

fn test_project() -> Result<(git2::Repository, projects::Project)> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(&path)?;
    let repo = git2::Repository::init(&path)?;
    // make init commit to the repo
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let sig = repo.signature()?;
    let _commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "initial commit",
        &repo.find_tree(oid)?,
        &[],
    )?;

    let project = projects::Project::from_path(path)?;
    Ok((repo, project))
}

#[test]
fn test_current_none() {
    let (repo, project) = test_project().unwrap();
    let current_session = super::sessions::Session::current(&repo, &project);
    assert!(current_session.is_ok());
    assert!(current_session.unwrap().is_none());
}

#[test]
fn test_create_current() {
    let (repo, project) = test_project().unwrap();
    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_ok());
    assert!(current_session.as_ref().unwrap().id.len() > 0);
    assert_eq!(current_session.as_ref().unwrap().hash, None);
    assert!(current_session.as_ref().unwrap().meta.start_timestamp_ms > 0);
    assert_eq!(
        current_session.as_ref().unwrap().meta.last_timestamp_ms,
        current_session.as_ref().unwrap().meta.start_timestamp_ms
    );
    assert_eq!(
        current_session.as_ref().unwrap().meta.branch,
        "refs/heads/master"
    );
    assert!(current_session.as_ref().unwrap().meta.commit.len() > 0);
    assert_eq!(current_session.as_ref().unwrap().activity.len(), 0);
}

#[test]
fn test_get_current() {
    let (repo, project) = test_project().unwrap();
    let created_session = super::sessions::Session::from_head(&repo, &project);
    assert!(created_session.is_ok());
    let created_session = created_session.unwrap();

    let current_session = super::sessions::Session::current(&repo, &project);
    assert!(current_session.is_ok());
    let current_session = current_session.unwrap();

    assert!(current_session.is_some());
    let current_session = current_session.unwrap();
    assert_eq!(current_session, created_session);
}

#[test]
fn test_flush() {
    let (repo, project) = test_project().unwrap();
    let created_session = super::sessions::Session::from_head(&repo, &project);
    assert!(created_session.is_ok());
    let mut created_session = created_session.unwrap();

    let flush_result = created_session.flush(&repo, &None, &project);
    assert!(flush_result.is_ok());
    assert!(created_session.hash.is_some());

    let current_session = super::sessions::Session::current(&repo, &project);
    assert!(current_session.is_ok());
    let current_session = current_session.unwrap();
    assert!(current_session.is_none());
}

#[test]
fn test_list() {
    let (repo, project) = test_project().unwrap();
    let first = super::sessions::Session::from_head(&repo, &project);
    assert!(first.is_ok());
    let mut first = first.unwrap();
    first.flush(&repo, &None, &project).unwrap();
    assert!(first.hash.is_some());

    std::thread::sleep(std::time::Duration::from_millis(1));

    let second = super::sessions::Session::from_head(&repo, &project);
    assert!(second.is_ok());
    let mut second = second.unwrap();
    second.flush(&repo, &None, &project).unwrap();
    assert!(second.hash.is_some());

    std::thread::sleep(std::time::Duration::from_millis(1));

    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_ok());
    let current = current_session.unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let sessions = super::sessions::list(&repo, &project, &reference);
    assert!(sessions.is_ok());
    let sessions = sessions.unwrap();

    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions[0], current);
    assert_eq!(sessions[1], second);
    assert_eq!(sessions[2], first);
}
