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

    let plan = gitbutler_filemonitor::compute_watch_plan(worktree).expect("plan computed");
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
