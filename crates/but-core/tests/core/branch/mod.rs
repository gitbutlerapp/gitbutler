mod canned_refname;
mod find_unique_refname;
mod normalize_short_name;
mod refuse_colliding_refname;

mod safe_delete {
    use but_core::branch;
    use but_testsupport::{gix_testtools, gix_testtools::Creation};

    #[test]
    fn main_and_linked_callers_find_each_occupied_worktree_once() -> anyhow::Result<()> {
        let root = gix_testtools::scripted_fixture_read_only("delete-references.sh")
            .map_err(anyhow::Error::from_boxed)?;
        for caller in ["prime", "worktree-one"] {
            let repo = gix::open_opts(root.join(caller), gix::open::Options::isolated())?;
            let state = branch::SafeDelete::new(&repo)?;
            for (name, worktree) in [
                ("main", "prime"),
                ("worktree-one", "worktree-one"),
                ("worktree-without-checkout", "worktree-without-checkout"),
            ] {
                let reference = repo.find_reference(name)?;
                let paths = state
                    .worktree_dirs_with_ref(&reference)
                    .expect("every occupied branch remains protected");
                assert_eq!(paths.len(), 1, "{caller}: each worktree is counted once");
                assert_eq!(
                    paths[0].file_name().expect("worktree path has a name"),
                    worktree,
                    "{caller}: the branch identifies its occupied worktree"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn delete_reference_journey() -> anyhow::Result<()> {
        let root = gix_testtools::scripted_fixture_writable_with_args(
            "delete-references.sh",
            None::<String>,
            Creation::Execute,
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
}
