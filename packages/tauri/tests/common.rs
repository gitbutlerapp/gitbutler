use gitbutler::git;

pub fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("failed to create temp dir")
        .into_path()
}

pub fn git_repository() -> git::Repository {
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
