use std::{fs, path};

use tempfile::tempdir;

use crate::{database, git};

pub fn test_database() -> database::Database {
    let path = temp_dir().join("test.db");
    database::Database::try_from(&path).unwrap()
}

pub fn temp_dir() -> path::PathBuf {
    let path = tempdir().unwrap().path().to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn test_repository() -> git::Repository {
    let path = temp_dir();
    let repository = git::Repository::init(path).expect("failed to init repository");
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git::Signature::now("test", "test@email.com").unwrap();
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

pub fn commit_all(repository: &git::Repository) -> git::Oid {
    let mut index = repository.index().expect("failed to get index");
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .expect("failed to add all");
    index.write().expect("failed to write index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git::Signature::now("test", "test@email.com").unwrap();
    let commit_oid = repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "some commit",
            &repository.find_tree(oid).expect("failed to find tree"),
            &[&repository
                .find_commit(
                    repository
                        .refname_to_id("HEAD")
                        .expect("failed to get head"),
                )
                .expect("failed to find commit")],
        )
        .expect("failed to commit");
    commit_oid
}
