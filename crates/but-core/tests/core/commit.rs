use but_testsupport::gix_testtools;
use but_core::RepositoryExt;

#[test]
fn is_conflicted() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let normal = but_core::Commit::from_id(repo.rev_parse_single("normal")?)?;
    assert!(!normal.is_conflicted());

    let conflicted = but_core::Commit::from_id(repo.rev_parse_single("conflicted")?)?;
    assert!(conflicted.is_conflicted());
    Ok(())
}

#[test]
fn commit_signatures_with_fallback() -> anyhow::Result<()> {
    // Use gix-testtools to create a test repository
    let temp_dir = but_testsupport::gix_testtools::tempfile::tempdir()?;
    let repo_path = temp_dir.path().join("test_repo");
    
    // Initialize repository using gix (avoiding git2 dependency)
    let _repo = gix::init_bare(&repo_path)?;
    
    // Configure the repo to be non-bare by adding a work tree
    let repo = gix::open_opts(&repo_path, gix::open::Options::isolated())?;
    
    // Verify no author is configured initially
    assert!(repo.author().transpose()?.is_none(), "Repository should have no author configured");
    
    // Test fallback functionality - should not fail even without configured author
    let signatures = repo.commit_signatures_with_fallback()?;
    let (author, committer) = signatures;
    
    // Verify we got GitButler fallback signatures
    assert_eq!(author.name, "GitButler");
    assert_eq!(author.email, "gitbutler@gitbutler.com");
    assert_eq!(committer.name, "GitButler");
    assert_eq!(committer.email, "gitbutler@gitbutler.com");
    
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
