use super::*;

use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

use notify::RecursiveMode;

type CanonicalWatch = (&'static str, String);

#[test]
fn compute_watch_plan_respects_gitignore_for_ignored_dirs() {
    let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignores-node-modules");
    let worktree = repo.workdir().expect("non-bare");

    let plan = compute_watch_plan(worktree).expect("plan computed");
    let worktree_real = gix::path::realpath(worktree).expect("realpath worktree");

    let actual = canonicalize_plan(&plan, &worktree_real);
    let expected: BTreeSet<CanonicalWatch> = [
        ("non-recursive", "."),
        ("non-recursive", ".git"),
        ("non-recursive", ".git/logs"),
        ("recursive", ".git/refs/heads"),
        ("non-recursive", "src"),
    ]
    .into_iter()
    .map(|(mode, path)| (mode, path.to_string()))
    .collect();

    assert_eq!(actual, expected);
}

#[test]
fn compute_watch_plan_includes_ignored_but_tracked_dirs() {
    let (repo, _tmpdir) = but_testsupport::writable_scenario("watch-plan-ignored-but-tracked");
    let worktree = repo.workdir().expect("non-bare");

    let plan = compute_watch_plan(worktree).expect("plan computed");
    let worktree_real = gix::path::realpath(worktree).expect("realpath worktree");

    let actual = canonicalize_plan(&plan, &worktree_real);
    let expected: BTreeSet<CanonicalWatch> = [
        ("non-recursive", "."),
        ("non-recursive", ".git"),
        ("non-recursive", ".git/logs"),
        ("recursive", ".git/refs/heads"),
        ("non-recursive", "ignored_dir"),
        ("non-recursive", "normal_dir"),
    ]
    .into_iter()
    .map(|(mode, path)| (mode, path.to_string()))
    .collect();

    assert_eq!(actual, expected);
}

#[test]
fn compute_watch_plan_prioritizes_git_watches() {
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
    plan: &[(std::path::PathBuf, RecursiveMode)],
    worktree: &Path,
) -> BTreeSet<CanonicalWatch> {
    plan.iter()
        .map(|(path, mode)| {
            let mode = match mode {
                RecursiveMode::Recursive => "recursive",
                RecursiveMode::NonRecursive => "non-recursive",
            };
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
            let path = if components.is_empty() {
                ".".to_string()
            } else {
                components.join("/")
            };
            (mode, path)
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
///
/// This is public to allow deterministic tests of the watch setup without relying on platform
/// specific filesystem notification behaviour.
fn compute_watch_plan(
    worktree_path: &Path,
) -> anyhow::Result<Vec<(PathBuf, notify::RecursiveMode)>> {
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
