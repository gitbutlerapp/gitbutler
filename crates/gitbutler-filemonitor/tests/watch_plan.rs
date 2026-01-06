use std::fs;

use notify::RecursiveMode;

#[test]
fn compute_watch_plan_avoids_recursive_worktree_watch_and_ignored_dirs() {
    let tmp = tempfile::tempdir().expect("tempdir available");
    let worktree = tmp.path();
    gix::init(worktree).expect("git repo initializes");

    fs::write(worktree.join(".gitignore"), "node_modules/\n").expect("write .gitignore");

    fs::create_dir_all(worktree.join("src")).expect("create src");
    fs::write(worktree.join("src").join("app.txt"), "hi").expect("write file");

    fs::create_dir_all(worktree.join("node_modules").join("pkg")).expect("create ignored dir");
    fs::write(worktree.join("node_modules").join("pkg").join("index.js"), "x")
        .expect("write ignored file");

    let plan = gitbutler_filemonitor::compute_watch_plan(worktree).expect("plan computed");
    let worktree_real = gix::path::realpath(worktree).expect("realpath worktree");
    let src_real = gix::path::realpath(worktree.join("src")).expect("realpath src");
    let node_modules_real =
        gix::path::realpath(worktree.join("node_modules")).expect("realpath node_modules");

    assert!(
        !plan
            .iter()
            .any(|(path, mode)| path == &worktree_real && *mode == RecursiveMode::Recursive),
        "worktree should not be watched recursively"
    );
    assert!(
        plan.iter().any(|(path, mode)| path == &worktree_real && *mode == RecursiveMode::NonRecursive),
        "worktree should be watched non-recursively"
    );
    assert!(
        plan.iter().any(|(path, mode)| path == &src_real && *mode == RecursiveMode::NonRecursive),
        "unignored directories should be watched"
    );
    assert!(
        !plan.iter().any(|(path, _mode)| path == &node_modules_real),
        "ignored directories should not be watched"
    );
}
