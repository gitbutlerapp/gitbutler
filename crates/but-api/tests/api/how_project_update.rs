use std::{fs, path::Path};

use but_testsupport::{CommandExt as _, git_at_dir, gix_testtools::tempfile::TempDir};

fn init_repository(repo_path: &Path) {
    git_at_dir(repo_path).args(["init", "-b", "main"]).run();
    git_at_dir(repo_path)
        .args(["config", "user.email", "user@example.com"])
        .run();
    git_at_dir(repo_path)
        .args(["config", "user.name", "User"])
        .run();
    git_at_dir(repo_path)
        .args(["config", "gitbutler.signCommits", "false"])
        .run();
}

fn commit_file(repo_path: &Path, name: &str, contents: &str, message: &str) {
    fs::write(repo_path.join(name), contents).expect("write test file");
    git_at_dir(repo_path).args(["add", "--all"]).run();
    git_at_dir(repo_path)
        .args(["commit", "--no-gpg-sign", "--message", message])
        .run();
}

#[test]
fn prepare_project_update_uses_current_branch_and_pull_rebase() -> anyhow::Result<()> {
    let local = TempDir::new()?;
    let remote = TempDir::new()?;
    let other = TempDir::new()?;
    init_repository(local.path());
    commit_file(local.path(), "notes.md", "initial\n", "Initial");
    git_at_dir(remote.path()).args(["init", "--bare"]).run();
    git_at_dir(local.path())
        .args(["remote", "add", "origin", remote.path().to_str().unwrap()])
        .run();
    git_at_dir(local.path())
        .args(["push", "-u", "origin", "main"])
        .run();

    git_at_dir(other.path())
        .args(["clone", remote.path().to_str().unwrap(), "."])
        .run();
    git_at_dir(other.path())
        .args(["config", "user.email", "user@example.com"])
        .run();
    git_at_dir(other.path())
        .args(["config", "user.name", "User"])
        .run();
    commit_file(other.path(), "remote.md", "remote\n", "Remote");
    git_at_dir(other.path()).args(["push"]).run();
    git_at_dir(local.path()).args(["fetch", "origin"]).run();

    commit_file(local.path(), "local.md", "local\n", "Local");
    let ctx = but_ctx::Context::from_repo(gix::open(local.path())?)?.with_memory_app_cache();

    let update = but_api::how::how_prepare_project_update(&ctx)?;

    assert_eq!(update.branch_ref_name, "refs/heads/main");
    assert_eq!(
        update.integration.divergence.branch_ref_name.full,
        "refs/heads/main"
    );
    assert_eq!(
        update.integration.divergence.upstream_ref_name.full,
        "refs/remotes/origin/main"
    );
    assert_eq!(update.integration.divergence.local_only.len(), 1);
    assert_eq!(update.integration.divergence.upstream_only.len(), 1);
    assert_eq!(
        update.integration.integration.steps.len(),
        2,
        "pull-rebase should pick the upstream commit and then the local commit"
    );
    Ok(())
}
