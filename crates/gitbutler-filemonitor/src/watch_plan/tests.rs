mod compute_watch_plan {
    use std::path::{Component, Path, PathBuf};

    use gix::bstr::ByteSlice;
    use notify::RecursiveMode;

    use crate::watch_plan::compute_watch_plan_for_repo;

    type CanonicalWatch = (RecursiveMode, PathBuf);

    #[test]
    fn respects_gitignore_for_ignored_dirs() {
        let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignores-node-modules");
        let actual = compute_canonicalized_plan(&repo);
        // There is no node-modules here, and submodules are tracked just by their directories and special directories
        // It's OK for a directory to not exist, as `.git/logs` doesn't exist yet despite being a directory we want to add.
        insta::assert_debug_snapshot!(actual, @r#"
        [
            (
                NonRecursive,
                "",
            ),
            (
                NonRecursive,
                ".git",
            ),
            (
                Recursive,
                ".git/refs/heads",
            ),
            (
                NonRecursive,
                "submodule-worktree",
            ),
            (
                NonRecursive,
                "submodule-worktree/dir",
            ),
            (
                NonRecursive,
                "submodule-repo",
            ),
            (
                NonRecursive,
                "submodule-repo/dir",
            ),
            (
                NonRecursive,
                "submodule-repo/.git",
            ),
            (
                NonRecursive,
                "submodule-repo/.git/logs",
            ),
            (
                Recursive,
                "submodule-repo/.git/refs/heads",
            ),
            (
                NonRecursive,
                "src",
            ),
        ]
        "#);
    }

    #[test]
    fn includes_ignored_but_tracked_dirs() -> anyhow::Result<()> {
        let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignored-but-tracked");
        let actual = compute_canonicalized_plan(&repo);

        // We want to see ignored directories that are tracked, `ignored_but_tracked` and `submodule-worktree`
        // as these are always tracked.
        insta::assert_snapshot!(
            std::fs::read(
                repo.workdir_path(".gitignore")
                    .expect("worktree")
            )?.as_bstr(),
            @"
        ignored_but_tracked_dir/
        submodule-repo/
        submodule-worktree/
        "
        );
        insta::assert_debug_snapshot!(actual, @r#"
        [
            (
                NonRecursive,
                "",
            ),
            (
                NonRecursive,
                ".git",
            ),
            (
                NonRecursive,
                ".git/logs",
            ),
            (
                Recursive,
                ".git/refs/heads",
            ),
            (
                NonRecursive,
                "submodule-worktree",
            ),
            (
                NonRecursive,
                "normal_dir",
            ),
            (
                NonRecursive,
                "ignored_but_tracked_dir",
            ),
        ]
        "#);

        Ok(())
    }

    #[test]
    fn prioritizes_git_watches() {
        let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignores-node-modules");
        let worktree = repo.workdir().expect("non-bare");

        let plan = compute_watch_plan(worktree).expect("plan computed");
        let worktree_real = gix::path::realpath(worktree).expect("realpath worktree");

        let relative_paths: Vec<_> = plan
            .iter()
            .map(|(path, _mode)| canonicalize_one_path(path, &worktree_real))
            .collect();

        let first_non_git = relative_paths
            .iter()
            .position(|p| p != "." && !p.starts_with(".git"))
            .expect("fixture has at least one non-git directory watch");
        let last_git = relative_paths
            .iter()
            .rposition(|p| p.starts_with(".git"))
            .expect("plan watches git paths");

        assert!(
            last_git < first_non_git,
            "git watches should be added before worktree directory watches to preserve critical git monitoring when OS watch limits are hit; plan order was: {relative_paths:?}"
        );
    }

    fn canonicalize_plan(
        plan: Vec<(PathBuf, RecursiveMode)>,
        worktree: &Path,
    ) -> Vec<CanonicalWatch> {
        plan.into_iter()
            .map(|(path, mode)| {
                let relative = path
                    .strip_prefix(worktree)
                    .expect("fixture repositories keep all watched paths in the worktree");
                (mode, relative.to_owned())
            })
            .collect()
    }

    fn canonicalize_one_path(path: &Path, worktree: &Path) -> String {
        let relative = path
            .strip_prefix(worktree)
            .expect("fixture repositories keep all watched paths in the worktree");
        let mut components = Vec::new();
        for comp in relative.components() {
            let Component::Normal(part) = comp else {
                continue;
            };
            components.push(part.to_string_lossy());
        }
        if components.is_empty() {
            ".".to_string()
        } else {
            components.join("/")
        }
    }

    /// Compute the paths that should be watched for `worktree_path`.
    fn compute_watch_plan(worktree_path: &Path) -> anyhow::Result<Vec<(PathBuf, RecursiveMode)>> {
        let worktree_path = gix::path::realpath(worktree_path)?;
        let repo = gix::open_opts(&worktree_path, gix::open::Options::isolated())?;
        let git_dir = repo.path().to_owned();
        let mut plan = Vec::new();
        compute_watch_plan_for_repo(&repo, &worktree_path, &git_dir, |path, mode| {
            plan.push((path.to_owned(), mode));
            Ok(std::ops::ControlFlow::Continue(()))
        })?;
        Ok(plan)
    }

    fn compute_canonicalized_plan(repo: &gix::Repository) -> Vec<CanonicalWatch> {
        let worktree = repo.workdir().expect("non-bare");
        let plan = compute_watch_plan(worktree).expect("plan computed");
        let worktree_real = gix::path::realpath(worktree).expect("realpath worktree");
        canonicalize_plan(plan, &worktree_real)
    }
}
