use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn log_target_first_parent_stops_gracefully_in_shallow_repo() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let remote_dir = tmp.path().join("remote");
    std::fs::create_dir(&remote_dir)?;

    // Create a remote repo with several commits.
    git_at_dir(&remote_dir)
        .args(["init", "-b", "master", "--object-format=sha1"])
        .run();
    std::fs::write(remote_dir.join("file"), "initial\n")?;
    git_at_dir(&remote_dir).args(["add", "file"]).run();
    git_at_dir(&remote_dir)
        .args(["commit", "-m", "initial"])
        .run();
    for i in 1..=5 {
        std::fs::write(remote_dir.join("file"), format!("line {i}\n"))?;
        git_at_dir(&remote_dir)
            .args(["commit", "-am", &format!("commit {i}")])
            .run();
    }

    // Shallow-clone with depth 1 — only the tip commit is available locally.
    // Use file:// protocol because --depth is ignored for local path clones.
    let clone_dir = tmp.path().join("clone");
    let remote_url = format!("file://{}", remote_dir.display());
    git_at_dir(tmp.path())
        .args(["clone", "--depth", "1", &remote_url])
        .arg(&clone_dir)
        .run();

    let repo = open_repo(&clone_dir)?;
    let ctx = but_ctx::Context::from_repo(repo)?;
    let repo = ctx.repo.get()?;
    let head_id = repo.head_id()?;

    // With last_commit_id = Some(head), the function walks the parent chain.
    // In a shallow clone the parent objects are missing — the function should
    // stop gracefully instead of returning an error.
    let commits = but_workspace::legacy::log_target_first_parent(&ctx, Some(head_id.into()), 100)?;

    // The shallow clone has depth 1 so the head commit's parent is not available.
    // The traversal should return an empty or very short list (not the full history).
    assert!(
        commits.len() <= 1,
        "expected at most 1 commit from shallow traversal, got {}",
        commits.len()
    );
    Ok(())
}
