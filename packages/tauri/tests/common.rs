use gitbutler::git;

pub fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("failed to create temp dir")
        .into_path()
}

pub struct TestProject {
    local_repository: git::Repository,
}

impl Default for TestProject {
    fn default() -> Self {
        let path = temp_dir();
        let local_repository =
            git::Repository::init(path.clone()).expect("failed to init repository");
        let mut index = local_repository.index().expect("failed to get index");
        let oid = index.write_tree().expect("failed to write tree");
        let signature = git::Signature::now("test", "test@email.com").unwrap();
        local_repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Initial commit",
                &local_repository
                    .find_tree(oid)
                    .expect("failed to find tree"),
                &[],
            )
            .expect("failed to commit");

        let remote_path = temp_dir();
        let remote_repository = git::Repository::init_opts(
            remote_path,
            git2::RepositoryInitOptions::new()
                .bare(true)
                .external_template(false),
        )
        .expect("failed to init repository");

        {
            let mut remote = local_repository
                .remote(
                    "origin",
                    remote_repository
                        .path()
                        .to_str()
                        .expect("failed to convert path to str"),
                )
                .expect("failed to add remote");
            remote
                .push(&["refs/heads/master:refs/heads/master"], None)
                .expect("failed to push");
        }

        Self { local_repository }
    }
}

impl TestProject {
    pub fn path(&self) -> &std::path::Path {
        self.local_repository.workdir().unwrap()
    }

    pub fn push(&self) {
        let mut origin = self.local_repository.find_remote("origin").unwrap();
        origin
            .push(&["refs/heads/master:refs/heads/master"], None)
            .unwrap();
    }

    pub fn reset_hard(&self, oid: git::Oid) {
        let commit = self.local_repository.find_commit(oid).unwrap();
        self.local_repository
            .reset(&commit, git2::ResetType::Hard, None)
            .unwrap();
    }

    pub fn find_commit(&self, oid: git::Oid) -> Result<git::Commit, git::Error> {
        self.local_repository.find_commit(oid)
    }

    pub fn commit_all(&self, message: &str) -> git::Oid {
        let mut index = self.local_repository.index().expect("failed to get index");
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add all");
        index.write().expect("failed to write index");
        let oid = index.write_tree().expect("failed to write tree");
        let signature = git::Signature::now("test", "test@email.com").unwrap();
        self.local_repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &self
                    .local_repository
                    .find_tree(oid)
                    .expect("failed to find tree"),
                &[&self
                    .local_repository
                    .find_commit(
                        self.local_repository
                            .refname_to_id("HEAD")
                            .expect("failed to get head"),
                    )
                    .expect("failed to find commit")],
            )
            .expect("failed to commit")
    }

    pub fn references(&self) -> Vec<git::Reference> {
        self.local_repository
            .references()
            .expect("failed to get references")
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to read references")
    }
}
