use crate::{deltas::operations::Operation, projects};
use anyhow::Result;
use std::path::Path;
use tempfile::tempdir;

fn test_project() -> Result<projects::Project> {
    let path = tempdir()?.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(&path)?;
    git2::Repository::init(&path)?;
    let project = projects::Project::from_path(path)?;
    Ok(project)
}

#[test]
fn test_read_none() {
    let project = test_project().unwrap();
    let file_path = Path::new("test.txt");
    let deltas = super::read(&project, file_path);
    assert!(deltas.is_ok());
    assert!(deltas.unwrap().is_none());
}

#[test]
fn test_read_invalid() {
    let project = test_project().unwrap();
    let file_path = Path::new("test.txt");
    let full_file_path = project.deltas_path().join(file_path);

    std::fs::create_dir_all(full_file_path.parent().unwrap()).unwrap();
    std::fs::write(full_file_path, "invalid").unwrap();

    let deltas = super::read(&project, file_path);
    assert!(deltas.is_err());
}

#[test]
fn test_write_read() {
    let project = test_project().unwrap();
    let file_path = Path::new("test.txt");

    let deltas = vec![super::Delta {
        operations: vec![Operation::Insert((0, "Hello, world!".to_string()))],
        timestamp_ms: 0,
    }];
    let write_result = super::write(&project, file_path, &deltas);
    assert!(write_result.is_ok());

    let read_result = super::read(&project, file_path);
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap().unwrap(), deltas);
}
