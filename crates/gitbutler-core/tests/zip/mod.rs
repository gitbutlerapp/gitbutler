use std::{fs::File, io::Write};

use gitbutler_core::zip::Zipper;
use tempfile::tempdir;
use walkdir::WalkDir;

#[test]
fn zip_dir() {
    let tmp_dir = tempdir().unwrap();
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("test.txt");
    let mut file = File::create(file_path).unwrap();
    file.write_all(b"test").unwrap();

    let zipper_cache = tempdir().unwrap();
    let zipper = Zipper::new(zipper_cache.path());
    let zip_file_path = zipper.zip(tmp_dir).unwrap();
    assert!(zip_file_path.exists());
}

#[test]
fn zip_file() {
    let tmp_dir = tempdir().unwrap();
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"test").unwrap();

    let zipper_cache = tempdir().unwrap();
    let zipper = Zipper::new(zipper_cache.path());
    zipper.zip(file_path).unwrap_err();
}

#[test]
fn zip_once() {
    let tmp_dir = tempdir().unwrap();
    let tmp_dir_path = tmp_dir.path();
    let file_path = tmp_dir_path.join("test.txt");
    let mut file = File::create(file_path).unwrap();
    file.write_all(b"test").unwrap();

    let zipper_cache = tempdir().unwrap();
    let zipper = Zipper::new(zipper_cache.path());
    assert_eq!(zipper.zip(&tmp_dir).unwrap(), zipper.zip(&tmp_dir).unwrap());
    assert_eq!(WalkDir::new(tmp_dir).into_iter().count(), 1);
}
