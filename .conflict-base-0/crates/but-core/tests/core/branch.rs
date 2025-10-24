use but_core::branch;
use but_testsupport::gix_testtools;
use but_testsupport::gix_testtools::Creation;

#[test]
fn delete_reference_journey() -> anyhow::Result<()> {
    let root = gix_testtools::scripted_fixture_writable_with_args(
        "delete-references.sh",
        None::<String>,
        Creation::ExecuteScript,
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = gix::open_opts(root.path().join("prime"), gix::open::Options::isolated())?;

    let state = branch::SafeDelete::new(&repo)?;

    // main is checked out in the prime repo
    let out = state.delete_reference(&repo.find_reference("main")?)?;
    assert!(!out.was_deleted());
    assert_eq!(
        out.checked_out_in_worktree_dirs
            .unwrap()
            .first()
            .unwrap()
            .file_name()
            .unwrap(),
        "prime",
        "the prime repository has checked it out"
    );

    let out = state.delete_reference(&repo.find_reference("not-checked-out")?)?;
    assert!(out.was_deleted());

    for worktree_name in ["worktree-one", "worktree-without-checkout"] {
        let out = state.delete_reference(&repo.find_reference(worktree_name)?)?;
        assert!(
            !out.was_deleted(),
            "worktree refs are never deleted, even if the worktree doesn't exist on disk"
        );
    }

    Ok(())
}
