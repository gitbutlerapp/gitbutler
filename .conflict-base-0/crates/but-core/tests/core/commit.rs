use but_testsupport::gix_testtools;

#[test]
fn is_conflicted() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let normal = but_core::Commit::from_id(repo.rev_parse_single("normal")?)?;
    assert!(!normal.is_conflicted());

    let conflicted = but_core::Commit::from_id(repo.rev_parse_single("conflicted")?)?;
    assert!(conflicted.is_conflicted());
    Ok(())
}

pub fn conflict_repo(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("conflict-commits.sh")
        .map_err(anyhow::Error::from_boxed)?;
    Ok(gix::open_opts(
        root.join(name),
        gix::open::Options::isolated(),
    )?)
}
