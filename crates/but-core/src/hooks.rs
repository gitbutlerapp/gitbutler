use anyhow::Context as _;

/// Run Git's `post-checkout` hooks after a branch checkout in `workdir`.
///
/// Pass the previous and current HEAD commit IDs; for a newly created worktree,
/// `old_head` is the null object ID. The branch-checkout flag is always `1`.
/// Git handles hook discovery, including `core.hooksPath` and config-based hooks.
/// Missing hooks succeed; launch failures and nonzero exits return an error.
/// Output is captured, with stderr included on failure. This does not roll back
/// checkout or invalidate caller-owned repository caches, even on failure.
pub fn post_checkout(
    workdir: &std::path::Path,
    old_head: gix::ObjectId,
    new_head: gix::ObjectId,
) -> anyhow::Result<()> {
    let output = std::process::Command::new(gix::path::env::exe_invocation())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-C")
        .arg(workdir)
        .args(["hook", "run", "--ignore-missing", "post-checkout", "--"])
        .arg(old_head.to_string())
        .arg(new_head.to_string())
        .arg("1")
        .output()
        .context("Failed to run post-checkout hook")?;
    anyhow::ensure!(
        output.status.success(),
        "Post-checkout hook failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}
