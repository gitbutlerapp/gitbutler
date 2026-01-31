/// Tests for worktree creation and management
mod util {
    use but_ctx::Context;
    use but_testsupport::gix_testtools::tempfile::TempDir;
    use gitbutler_stack::VirtualBranchesHandle;

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (repo, tmpdir) = but_testsupport::writable_scenario(name);
        // TODO: all this should work without `Context` once it's switched to the new rebase engine,
        //       making this crate either obsolete or proper plumbing.
        let mut ctx = Context::from_repo(repo)?;
        // update the vb-toml metadata - trigger reconciliation and write the vb.toml according to what's there.
        {
            let _guard = ctx.exclusive_worktree_access();
            let meta = ctx.legacy_meta()?;
            meta.write_reconciled(&*ctx.repo.get()?)?;
        }
        let handle = VirtualBranchesHandle::new(ctx.project_data_dir());

        Ok(TestContext {
            ctx,
            handle,
            tmpdir,
        })
    }

    #[allow(unused)]
    pub struct TestContext {
        pub ctx: Context,
        pub handle: VirtualBranchesHandle,
        pub tmpdir: TempDir,
    }
}

mod worktree_new;

mod worktree_list {
    use but_worktrees::{list::worktree_list, new::worktree_new};

    use crate::util::test_ctx;

    #[test]
    fn can_list_worktrees() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let guard = ctx.exclusive_worktree_access();

        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
        let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let b = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let c = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let d = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let e = worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;

        let all = &[&a, &b, &c, &d, &e];

        // All should start normal
        assert!(
            worktree_list(&mut ctx, guard.read_permission())?
                .entries
                .iter()
                .all(|e| all.iter().any(|a| a.created == *e))
        );

        Ok(())
    }
}

mod various;
mod worktree_destroy;
