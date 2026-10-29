use anyhow::Context as _;

/// Run configured and traditional `post-checkout` hooks after a branch checkout in `repo`.
/// Hook failures are reported after all enabled hooks have run, leaving the completed checkout intact.
pub fn post_checkout(
    repo: &gix::Repository,
    old_head: gix::ObjectId,
    new_head: gix::ObjectId,
) -> anyhow::Result<()> {
    let old_head = old_head.to_string();
    let new_head = new_head.to_string();
    let outputs = repo
        .run_hook_event_serially(
            "post-checkout",
            gix::hook::Options {
                arguments: &[old_head.as_ref(), new_head.as_ref(), "1".as_ref()],
                environment: &[("GIT_INDEX_FILE".as_ref(), repo.index_path().as_os_str())],
                ..Default::default()
            },
        )
        .context("Failed to run post-checkout hook")?;
    if let Some(failed) = outputs.iter().find(|output| !output.status.success()) {
        let diagnostics: Vec<_> = outputs
            .iter()
            .flat_map(|output| output.stdout.iter().chain(&output.stderr))
            .copied()
            .collect();
        anyhow::bail!(
            "Post-checkout hook failed ({}): {}",
            failed.status,
            String::from_utf8_lossy(&diagnostics).trim()
        );
    }
    Ok(())
}
