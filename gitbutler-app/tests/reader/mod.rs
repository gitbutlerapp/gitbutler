use gitbutler_app::reader::{CommitReader, Content, Reader};
use std::fs;
use std::path::Path;

use crate::{commit_all, temp_dir, test_repository};
use anyhow::Result;

#[test]
fn directory_reader_read_file() -> Result<()> {
    let dir = temp_dir();

    let file_path = Path::new("test.txt");
    fs::write(dir.path().join(file_path), "test")?;

    let reader = Reader::open(dir.path())?;
    assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

    Ok(())
}

#[test]
fn commit_reader_read_file() -> Result<()> {
    let (repository, _tmp) = test_repository();

    let file_path = Path::new("test.txt");
    fs::write(repository.path().parent().unwrap().join(file_path), "test")?;

    let oid = commit_all(&repository);

    fs::write(repository.path().parent().unwrap().join(file_path), "test2")?;

    let reader = Reader::from_commit(&repository, &repository.find_commit(oid)?)?;
    assert_eq!(reader.read(file_path)?, Content::UTF8("test".to_string()));

    Ok(())
}

#[test]
fn reader_list_files_should_return_relative() -> Result<()> {
    let dir = temp_dir();

    fs::write(dir.path().join("test1.txt"), "test")?;
    fs::create_dir_all(dir.path().join("dir"))?;
    fs::write(dir.path().join("dir").join("test.txt"), "test")?;

    let reader = Reader::open(dir.path())?;
    let files = reader.list_files(Path::new("dir"))?;
    assert_eq!(files.len(), 1);
    assert!(files.contains(&Path::new("test.txt").to_path_buf()));

    Ok(())
}

#[test]
fn reader_list_files() -> Result<()> {
    let dir = temp_dir();

    fs::write(dir.path().join("test.txt"), "test")?;
    fs::create_dir_all(dir.path().join("dir"))?;
    fs::write(dir.path().join("dir").join("test.txt"), "test")?;

    let reader = Reader::open(dir.path())?;
    let files = reader.list_files(Path::new(""))?;
    assert_eq!(files.len(), 2);
    assert!(files.contains(&Path::new("test.txt").to_path_buf()));
    assert!(files.contains(&Path::new("dir/test.txt").to_path_buf()));

    Ok(())
}

#[test]
fn commit_reader_list_files_should_return_relative() -> Result<()> {
    let (repository, _tmp) = test_repository();

    fs::write(
        repository.path().parent().unwrap().join("test1.txt"),
        "test",
    )?;
    fs::create_dir_all(repository.path().parent().unwrap().join("dir"))?;
    fs::write(
        repository
            .path()
            .parent()
            .unwrap()
            .join("dir")
            .join("test.txt"),
        "test",
    )?;

    let oid = commit_all(&repository);

    fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

    let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
    let files = reader.list_files(Path::new("dir"))?;
    assert_eq!(files.len(), 1);
    assert!(files.contains(&Path::new("test.txt").to_path_buf()));

    Ok(())
}

#[test]
fn commit_reader_list_files() -> Result<()> {
    let (repository, _tmp) = test_repository();

    fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;
    fs::create_dir_all(repository.path().parent().unwrap().join("dir"))?;
    fs::write(
        repository
            .path()
            .parent()
            .unwrap()
            .join("dir")
            .join("test.txt"),
        "test",
    )?;

    let oid = commit_all(&repository);

    fs::remove_dir_all(repository.path().parent().unwrap().join("dir"))?;

    let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
    let files = reader.list_files(Path::new(""))?;
    assert_eq!(files.len(), 2);
    assert!(files.contains(&Path::new("test.txt").to_path_buf()));
    assert!(files.contains(&Path::new("dir/test.txt").to_path_buf()));

    Ok(())
}

#[test]
fn directory_reader_exists() -> Result<()> {
    let dir = temp_dir();

    fs::write(dir.path().join("test.txt"), "test")?;

    let reader = Reader::open(dir.path())?;
    assert!(reader.exists(Path::new("test.txt"))?);
    assert!(!reader.exists(Path::new("test2.txt"))?);

    Ok(())
}

#[test]
fn commit_reader_exists() -> Result<()> {
    let (repository, _tmp) = test_repository();

    fs::write(repository.path().parent().unwrap().join("test.txt"), "test")?;

    let oid = commit_all(&repository);

    fs::remove_file(repository.path().parent().unwrap().join("test.txt"))?;

    let reader = CommitReader::new(&repository, &repository.find_commit(oid)?)?;
    assert!(reader.exists(Path::new("test.txt")));
    assert!(!reader.exists(Path::new("test2.txt")));

    Ok(())
}

#[test]
fn from_bytes() {
    for (bytes, expected) in [
        ("test".as_bytes(), Content::UTF8("test".to_string())),
        (&[0, 159, 146, 150, 159, 146, 150], Content::Binary),
    ] {
        assert_eq!(Content::from(bytes), expected);
    }
}

#[test]
fn serialize_content() {
    for (content, expected) in [
        (
            Content::UTF8("test".to_string()),
            r#"{"type":"utf8","value":"test"}"#,
        ),
        (Content::Binary, r#"{"type":"binary"}"#),
        (Content::Large, r#"{"type":"large"}"#),
    ] {
        assert_eq!(serde_json::to_string(&content).unwrap(), expected);
    }
}
