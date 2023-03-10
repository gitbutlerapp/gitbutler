use std::path::Path;

use crate::{projects, users};
use anyhow::Result;
use tempfile::tempdir;

fn test_user() -> users::User {
    users::User {
        id: 0,
        name: "test".to_string(),
        email: "test@email.com".to_string(),
        picture: "test".to_string(),
        locale: None,
        created_at: "0".to_string(),
        updated_at: "0".to_string(),
        access_token: "0".to_string(),
    }
}

fn test_project_empty() -> Result<(git2::Repository, projects::Project)> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(&path)?;
    let repo = git2::Repository::init(&path)?;
    let project = projects::Project::from_path(path)?;
    Ok((repo, project))
}

fn test_project() -> Result<(git2::Repository, projects::Project)> {
    let (repo, project) = test_project_empty()?;
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let sig = git2::Signature::now("test", "test@email.com").unwrap();
    let _commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "initial commit",
        &repo.find_tree(oid)?,
        &[],
    )?;
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
fn test_create_current_fails_when_meta_path_exists() {
    let (repo, project) = test_project().unwrap();

    let meta_path = project.session_path().join("meta");
    std::fs::create_dir_all(&meta_path).unwrap();

    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_err());
}

#[test]
fn test_create_current_when_session_dir_exists() {
    let (repo, project) = test_project().unwrap();

    let session_dir = project.session_path();
    std::fs::create_dir_all(&session_dir).unwrap();

    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_ok());
}

#[test]
fn test_create_current_empty() {
    let (repo, project) = test_project_empty().unwrap();
    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_ok());
    assert!(current_session.as_ref().unwrap().id.len() > 0);
    assert_eq!(current_session.as_ref().unwrap().hash, None);
    assert!(current_session.as_ref().unwrap().meta.start_timestamp_ms > 0);
    assert_eq!(
        current_session.as_ref().unwrap().meta.last_timestamp_ms,
        current_session.as_ref().unwrap().meta.start_timestamp_ms
    );
    assert!(current_session.as_ref().unwrap().meta.branch.is_none());
    assert!(current_session.as_ref().unwrap().meta.commit.is_none());
    assert_eq!(current_session.as_ref().unwrap().activity.len(), 0);
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
    assert!(current_session.as_ref().unwrap().meta.branch.is_some());
    assert_eq!(
        current_session
            .as_ref()
            .unwrap()
            .meta
            .branch
            .as_ref()
            .unwrap(),
        "refs/heads/master"
    );
    assert!(current_session.as_ref().unwrap().meta.commit.is_some());
    assert_eq!(
        current_session
            .as_ref()
            .unwrap()
            .meta
            .commit
            .as_ref()
            .unwrap(),
        repo.head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
            .to_string()
            .as_str()
    );
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

    let head_commit = repo
        .find_reference(&project.refname())
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(head_commit.author().name().unwrap(), "gitbutler");
    assert_eq!(head_commit.author().email().unwrap(), "gitbutler@localhost");
    assert_eq!(head_commit.committer().name().unwrap(), "gitbutler");
    assert_eq!(
        head_commit.committer().email().unwrap(),
        "gitbutler@localhost"
    );

    let current_session = super::sessions::Session::current(&repo, &project);
    assert!(current_session.is_ok());
    let current_session = current_session.unwrap();
    assert!(current_session.is_none());
}

#[test]
fn test_flush_with_user() {
    let (repo, project) = test_project().unwrap();
    let created_session = super::sessions::Session::from_head(&repo, &project);
    assert!(created_session.is_ok());
    let mut created_session = created_session.unwrap();

    let flush_result = created_session.flush(&repo, &Some(test_user()), &project);
    assert!(flush_result.is_ok());
    assert!(created_session.hash.is_some());

    let head_commit = repo
        .find_reference(&project.refname())
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(head_commit.author().name().unwrap(), "test");
    assert_eq!(head_commit.author().email().unwrap(), "test@email.com");
    assert_eq!(head_commit.committer().name().unwrap(), "gitbutler");
    assert_eq!(
        head_commit.committer().email().unwrap(),
        "gitbutler@localhost"
    );

    let current_session = super::sessions::Session::current(&repo, &project);
    assert!(current_session.is_ok());
    let current_session = current_session.unwrap();
    assert!(current_session.is_none());
}

#[test]
fn test_get_persistent() {
    let (repo, project) = test_project().unwrap();
    let created_session = super::sessions::Session::from_head(&repo, &project);
    assert!(created_session.is_ok());
    let mut created_session = created_session.unwrap();

    created_session.flush(&repo, &None, &project).unwrap();

    let commid_oid = git2::Oid::from_str(&created_session.hash.as_ref().unwrap()).unwrap();
    let commit = repo.find_commit(commid_oid).unwrap();

    let reconstructed = super::sessions::Session::from_commit(&repo, &commit);
    assert!(reconstructed.is_ok());
    let reconstructed = reconstructed.unwrap();

    assert_eq!(reconstructed, created_session);
}

#[test]
fn test_list() {
    let (repo, project) = test_project().unwrap();
    let first = super::sessions::Session::from_head(&repo, &project);
    assert!(first.is_ok());
    let mut first = first.unwrap();
    first.flush(&repo, &None, &project).unwrap();
    assert!(first.hash.is_some());

    let second = super::sessions::Session::from_head(&repo, &project);
    assert!(second.is_ok());
    let mut second = second.unwrap();
    second.flush(&repo, &None, &project).unwrap();
    assert!(second.hash.is_some());

    let current_session = super::sessions::Session::from_head(&repo, &project);
    assert!(current_session.is_ok());
    let current = current_session.unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let sessions = super::sessions::list(&repo, &project, &reference);
    assert!(sessions.is_ok());
    let sessions = sessions.unwrap();

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0], current);
    assert_eq!(sessions[1], second);
    // NOTE: first session is not included in the list
}

#[test]
fn test_list_files_from_first_presistent_session() {
    let (repo, project) = test_project().unwrap();

    let file_path = Path::new(&project.path).join("test.txt");

    std::fs::write(file_path.clone(), "zero").unwrap();

    let first = super::sessions::Session::from_head(&repo, &project);
    assert!(first.is_ok());
    let mut first = first.unwrap();
    first.flush(&repo, &None, &project).unwrap();
    assert!(first.hash.is_some());

    let file_path = Path::new(&project.path).join("test.txt");
    std::fs::write(file_path.clone(), "one").unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let files = super::sessions::list_files(&repo, &project, &reference, &first.id, None);
    assert!(files.is_ok());
    let files = files.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files["test.txt"], "zero");
}

#[test]
fn test_list_files_from_second_current_session() {
    let (repo, project) = test_project().unwrap();

    let file_path = Path::new(&project.path).join("test.txt");
    std::fs::write(file_path.clone(), "zero").unwrap();

    let first = super::sessions::Session::from_head(&repo, &project);
    assert!(first.is_ok());
    let mut first = first.unwrap();
    first.flush(&repo, &None, &project).unwrap();
    assert!(first.hash.is_some());

    std::thread::sleep(std::time::Duration::from_millis(1));

    std::fs::write(file_path.clone(), "one").unwrap();

    let second = super::sessions::Session::from_head(&repo, &project);
    assert!(second.is_ok());
    let second = second.unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let files = super::sessions::list_files(&repo, &project, &reference, &second.id, None);
    assert!(files.is_ok());
    let files = files.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files["test.txt"], "zero");
}

#[test]
fn test_list_files_from_second_presistent_session() {
    let (repo, project) = test_project().unwrap();

    let file_path = Path::new(&project.path).join("test.txt");
    std::fs::write(file_path.clone(), "zero").unwrap();

    let first = super::sessions::Session::from_head(&repo, &project);
    assert!(first.is_ok());
    let mut first = first.unwrap();
    first.flush(&repo, &None, &project).unwrap();
    assert!(first.hash.is_some());

    std::thread::sleep(std::time::Duration::from_millis(1));

    std::fs::write(file_path.clone(), "one").unwrap();

    let second = super::sessions::Session::from_head(&repo, &project);
    assert!(second.is_ok());
    let mut second = second.unwrap();
    second.flush(&repo, &None, &project).unwrap();
    assert!(second.hash.is_some());

    std::fs::write(file_path.clone(), "two").unwrap();

    let reference = repo.find_reference(&project.refname()).unwrap();
    let files = super::sessions::list_files(&repo, &project, &reference, &second.id, None);
    assert!(files.is_ok());
    let files = files.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files["test.txt"], "zero");
}
