#[test]
fn change_2_to_two_in_second_commit() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_two")?;
    let tree_changes = but_core::diff::worktree_changes(&repo)?;
    dbg!(&tree_changes);
    Ok(())
}

#[test]
fn change_2_to_two_in_second_commit_after_shift_by_two() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10-shift_two")?;
    let tree_changes = but_core::diff::worktree_changes(&repo)?;
    dbg!(&tree_changes);
    Ok(())
}

fn repo(name: &str) -> anyhow::Result<gix::Repository> {
    let worktree_dir = gix_testtools::scripted_fixture_read_only("branch-states.sh")
        .map_err(anyhow::Error::from_boxed)?
        .join(name);
    Ok(gix::open_opts(
        worktree_dir,
        gix::open::Options::isolated(),
    )?)
}
