use std::{fs, path};

use tempfile::tempdir;

pub fn temp_dir() -> path::PathBuf {
    let path = tempdir().unwrap().path().to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn test_repository() -> git2::Repository {
    let path = temp_dir();
    let repository = git2::Repository::init(path).expect("failed to init repository");
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid).expect("failed to find tree"),
            &[],
        )
        .expect("failed to commit");
    repository
}
