use crate::projects;
use anyhow::Result;
use std::path::Path;
use tempfile::tempdir;

fn test_project() -> Result<(git2::Repository, projects::Project)> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(&path)?;
    let repo = git2::Repository::init(&path)?;
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
    let project = projects::Project::from_path(path)?;
    Ok((repo, project))
}

#[test]
fn test_flush_session() {
    let (repo, project) = test_project().unwrap();

    let relative_file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(relative_file_path), "hello").unwrap();

    let result = super::delta::register_file_change(&project, &repo, &relative_file_path);
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (mut session1, deltas1) = maybe_session_deltas.unwrap();
    assert_eq!(session1.hash, None);
    assert_eq!(deltas1.len(), 1);

    session1.flush(&repo, &None, &project).unwrap();
    assert!(session1.hash.is_some());

    std::fs::write(
        Path::new(&project.path).join(relative_file_path),
        "hello world",
    )
    .unwrap();

    let result = super::delta::register_file_change(&project, &repo, &relative_file_path);
    assert!(result.is_ok());
    let maybe_session_deltas = result.unwrap();
    assert!(maybe_session_deltas.is_some());
    let (mut session2, deltas2) = maybe_session_deltas.unwrap();
    assert_eq!(session2.hash, None);
    assert_eq!(deltas2.len(), 1);
    assert_ne!(session1.id, session2.id);

    session2.flush(&repo, &None, &project).unwrap();
    assert!(session2.hash.is_some());
}
