use std::{fs, path, path::PathBuf};

use but_oxidize::{git2_to_gix_object_id, gix_to_git2_oid};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_repo::RepositoryExt;
use tempfile::TempDir;

use crate::{VAR_NO_CLEANUP, init_opts};

pub fn temp_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

pub struct TestProject {
    pub local_repo: git2::Repository,
    local_tmp: Option<TempDir>,
    remote_repo: git2::Repository,
    remote_tmp: Option<TempDir>,
}

impl Drop for TestProject {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.local_tmp.take().map(|tmp| tmp.keep());
            let _ = self.remote_tmp.take().map(|tmp| tmp.keep());
        }
    }
}

impl Default for TestProject {
    fn default() -> Self {
        let local_tmp = temp_dir();
        let local_repository = git2::Repository::init_opts(local_tmp.path(), &init_opts())
            .expect("failed to init repository");
        setup_config(&local_repository.config().unwrap()).unwrap();
        let mut index = local_repository.index().expect("failed to get index");
        let oid = index.write_tree().expect("failed to write tree");
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        let repo: &git2::Repository = &local_repository;
        repo.commit_with_signature(
            Some(&"refs/heads/master".parse().unwrap()),
            &signature,
            &signature,
            "Initial commit",
            &local_repository
                .find_tree(oid)
                .expect("failed to find tree"),
            &[],
            None,
        )
        .expect("failed to commit");

        let remote_tmp = temp_dir();
        let remote_repository = git2::Repository::init_opts(
            remote_tmp.path(),
            git2::RepositoryInitOptions::new()
                .bare(true)
                .external_template(false),
        )
        .expect("failed to init repository");
        setup_config(&remote_repository.config().unwrap()).unwrap();

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

        Self {
            local_repo: local_repository,
            local_tmp: Some(local_tmp),
            remote_repo: remote_repository,
            remote_tmp: Some(remote_tmp),
        }
    }
}

impl TestProject {
    /// Take the tmp directory holding the local repository and make sure it won't be deleted,
    /// returning a path to it.
    /// Best used inside a `dbg!(test_project.debug_local_repo())`
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.local_tmp.take().map(|tmp| tmp.keep())
    }
    pub fn path(&self) -> &std::path::Path {
        self.local_repo.workdir().unwrap()
    }

    pub fn push_branch(&self, branch: &LocalRefname) {
        let mut origin = self.local_repo.find_remote("origin").unwrap();
        origin.push(&[&format!("{branch}:{branch}")], None).unwrap();
    }

    pub fn push(&self) {
        let mut origin = self.local_repo.find_remote("origin").unwrap();
        origin
            .push(&["refs/heads/master:refs/heads/master"], None)
            .unwrap();
    }

    /// ```text
    /// git add -A
    /// git reset --hard <oid>
    /// ```
    pub fn reset_hard(&self, oid: Option<git2::Oid>) {
        let mut index = self.local_repo.index().expect("failed to get index");
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add all");
        index.write().expect("failed to write index");

        let head = self.local_repo.head().unwrap();
        let commit = oid.map_or(head.peel_to_commit().unwrap(), |oid| {
            self.local_repo.find_commit(oid).unwrap()
        });

        let head_ref = head.name().unwrap();
        self.local_repo.find_reference(head_ref).unwrap();

        self.local_repo
            .reset(commit.as_object(), git2::ResetType::Hard, None)
            .unwrap();
    }

    /// fetch remote into local
    pub fn fetch(&self) {
        let mut remote = self.local_repo.find_remote("origin").unwrap();
        remote
            .fetch(&["+refs/heads/*:refs/remotes/origin/*"], None, None)
            .unwrap();
    }

    pub fn rebase_and_merge(&self, branch_name: &Refname) {
        let branch_name: Refname = match branch_name {
            Refname::Local(local) => format!("refs/heads/{}", local.branch()).parse().unwrap(),
            Refname::Remote(remote) => format!("refs/heads/{}", remote.branch()).parse().unwrap(),
            _ => "INVALID".parse().unwrap(), // todo
        };
        let branch = self
            .remote_repo
            .maybe_find_branch_by_refname(&branch_name)
            .unwrap();
        let branch_commit = branch.unwrap().get().peel_to_commit().unwrap();

        let master_branch = {
            let name: Refname = "refs/heads/master".parse().unwrap();
            self.remote_repo
                .maybe_find_branch_by_refname(&name)
                .unwrap()
        };
        let master_branch_commit = master_branch.unwrap().get().peel_to_commit().unwrap();

        let mut rebase_options = git2::RebaseOptions::new();
        rebase_options.quiet(true);
        rebase_options.inmemory(true);

        let mut rebase = self
            .remote_repo
            .rebase(
                Some(
                    &self
                        .remote_repo
                        .find_annotated_commit(branch_commit.id())
                        .unwrap(),
                ),
                Some(
                    &self
                        .remote_repo
                        .find_annotated_commit(master_branch_commit.id())
                        .unwrap(),
                ),
                None,
                Some(&mut rebase_options),
            )
            .unwrap();

        let mut rebase_success = true;
        let mut last_rebase_head = branch_commit.id();
        while let Some(Ok(op)) = rebase.next() {
            let commit = self.remote_repo.find_commit(op.id()).unwrap();
            let index = rebase.inmemory_index().unwrap();
            if index.has_conflicts() {
                rebase_success = false;
                break;
            }

            if let Ok(commit_id) = rebase.commit(None, &commit.committer(), None) {
                last_rebase_head = commit_id;
            } else {
                rebase_success = false;
                break;
            };
        }

        if rebase_success {
            self.remote_repo
                .reference(
                    "refs/heads/master",
                    last_rebase_head,
                    true,
                    &format!("rebase: {branch_name}"),
                )
                .unwrap();
        } else {
            rebase.abort().unwrap();
        }
    }

    /// works like if we'd open and merge a PR on github. does not update local.
    pub fn merge(&self, branch_name: &Refname) -> anyhow::Result<()> {
        let branch_name: Refname = match branch_name {
            Refname::Local(local) => format!("refs/heads/{}", local.branch()).parse()?,
            Refname::Remote(remote) => format!("refs/heads/{}", remote.branch()).parse()?,
            _ => "INVALID".parse()?, // todo
        };
        let branch = self
            .remote_repo
            .maybe_find_branch_by_refname(&branch_name)?
            .expect("branch exists");
        let branch_commit = branch.get().peel_to_commit()?;

        let master_branch = {
            let name: Refname = "refs/heads/master".parse()?;
            self.remote_repo.maybe_find_branch_by_refname(&name)?
        };
        let master_branch_commit = master_branch
            .as_ref()
            .expect("master branch exists")
            .get()
            .peel_to_commit()?;

        let gix_repo = gix::open_opts(self.remote_repo.path(), gix::open::Options::isolated())?;
        let merge_tree = {
            use but_core::RepositoryExt;
            let mut merge_result = gix_repo.merge_commits(
                git2_to_gix_object_id(master_branch_commit.id()),
                git2_to_gix_object_id(branch.get().peel_to_commit()?.id()),
                gix_repo.default_merge_labels(),
                gix::merge::commit::Options::default(),
            )?;
            assert!(
                !merge_result
                    .tree_merge
                    .has_unresolved_conflicts(Default::default()),
                "test-merges should have non-conflicting trees"
            );
            let tree_id = merge_result.tree_merge.tree.write()?;
            self.remote_repo.find_tree(gix_to_git2_oid(tree_id))?
        };

        let repo: &git2::Repository = &self.remote_repo;
        repo.commit_with_signature(
            Some(&"refs/heads/master".parse()?),
            &branch_commit.author(),
            &branch_commit.committer(),
            &format!("Merge pull request from {branch_name}"),
            &merge_tree,
            &[&master_branch_commit, &branch_commit],
            None,
        )?;
        Ok(())
    }

    pub fn find_commit(&self, oid: git2::Oid) -> Result<git2::Commit<'_>, git2::Error> {
        self.local_repo.find_commit(oid)
    }

    pub fn checkout_commit(&self, commit_oid: git2::Oid) {
        let commit = self.local_repo.find_commit(commit_oid).unwrap();
        let commit_tree = commit.tree().unwrap();

        self.local_repo.set_head_detached(commit_oid).unwrap();
        self.local_repo
            .checkout_tree_builder(&commit_tree)
            .force()
            .checkout()
            .unwrap();
    }

    pub fn checkout(&self, branch: &LocalRefname) {
        let refname: Refname = branch.into();
        let head_commit = self.local_repo.head().unwrap().peel_to_commit().unwrap();
        let tree = match self.local_repo.maybe_find_branch_by_refname(&refname) {
            Ok(branch) => match branch {
                Some(branch) => branch.get().peel_to_tree().unwrap(),
                None => {
                    self.local_repo
                        .reference(&refname.to_string(), head_commit.id(), false, "new branch")
                        .unwrap();
                    head_commit.tree().unwrap()
                }
            },
            // Ok(branch) => branch.get().peel_to_tree().unwrap(),
            // Err(err) if err.code() == git2::ErrorCode::NotFound => {
            //     self.local_repository
            //         .reference(&branch.to_string(), head_commit.id(), false, "new branch")
            //         .unwrap();
            //     head_commit.tree().unwrap()
            // }
            Err(error) => panic!("{error:?}"),
        };
        self.local_repo.set_head(&refname.to_string()).unwrap();
        self.local_repo
            .checkout_tree_builder(&tree)
            .force()
            .checkout()
            .unwrap();
    }

    /// takes all changes in the working directory and commits them into local
    pub fn commit_all(&self, message: &str) -> git2::Oid {
        let head = self.local_repo.head().unwrap();
        let mut index = self.local_repo.index().expect("failed to get index");
        index
            .add_all(["."], git2::IndexAddOption::DEFAULT, None)
            .expect("failed to add all");
        index.write().expect("failed to write index");
        let oid = index.write_tree().expect("failed to write tree");
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        let refname: Refname = head.name().unwrap().parse().unwrap();
        let repo: &git2::Repository = &self.local_repo;
        repo.commit_with_signature(
            Some(&refname),
            &signature,
            &signature,
            message,
            &self.local_repo.find_tree(oid).expect("failed to find tree"),
            &[&self
                .local_repo
                .find_commit(
                    self.local_repo
                        .refname_to_id("HEAD")
                        .expect("failed to get head"),
                )
                .expect("failed to find commit")],
            None,
        )
        .expect("failed to commit")
    }

    pub fn references(&self) -> Vec<git2::Reference<'_>> {
        self.local_repo
            .references()
            .expect("failed to get references")
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to read references")
    }

    pub fn add_submodule(&self, url: &gitbutler_url::Url, path: &path::Path) {
        let mut submodule = self
            .local_repo
            .submodule(&url.to_string(), path.as_ref(), false)
            .unwrap();
        let repo = submodule.open().unwrap();

        // checkout submodule's master head
        repo.find_remote("origin")
            .unwrap()
            .fetch(&["+refs/heads/*:refs/heads/*"], None, None)
            .unwrap();
        let reference = repo.find_reference("refs/heads/master").unwrap();
        let reference_head = repo.find_commit(reference.target().unwrap()).unwrap();
        repo.checkout_tree(reference_head.tree().unwrap().as_object(), None)
            .unwrap();

        // be sure that `HEAD` points to the actual head - `git2` seems to initialize it
        // with `init.defaultBranch`, causing failure otherwise.
        repo.set_head("refs/heads/master").unwrap();
        submodule.add_finalize().unwrap();
    }

    pub fn write_file(&self, path: &str, lines: &[String]) {
        fs::write(self.path().join(path), lines.join("\n")).unwrap()
    }

    pub fn gen_file(&self, path: &str, line_count: i32) -> Vec<String> {
        let lines: Vec<_> = (0_i32..line_count).map(|i| format!("line {i}")).collect();
        self.write_file(path, &lines);
        lines
    }
}

pub(crate) fn setup_config(config: &git2::Config) -> anyhow::Result<()> {
    match config.open_level(git2::ConfigLevel::Local) {
        Ok(mut local) => {
            local.set_str("commit.gpgsign", "false")?;
            local.set_str("user.name", "gitbutler-test")?;
            local.set_str("user.email", "gitbutler-test@example.com")?;
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}
