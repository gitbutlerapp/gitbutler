/// Tests for worktree creation and management
mod util {
    use gitbutler_command_context::CommandContext;
    use gitbutler_stack::VirtualBranchesHandle;
    use gix_testtools::tempfile::TempDir;

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (ctx, tmpdir) = gitbutler_testsupport::writable::but_fixture("worktree.sh", name)?;
        let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());

        Ok(TestContext {
            ctx,
            handle,
            tmpdir,
        })
    }

    #[allow(unused)]
    pub struct TestContext {
        pub ctx: CommandContext,
        pub handle: VirtualBranchesHandle,
        pub tmpdir: TempDir,
    }
}

mod worktree_new {
    use super::*;
    use anyhow::Context;
    use but_graph::VirtualBranchesTomlMetadata;
    use but_workspace::{StacksFilter, stacks_v3};
    use but_worktrees::new::worktree_new;
    use util::test_ctx;

    #[test]
    fn can_create_worktree_from_feature_a() -> anyhow::Result<()> {
        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let mut test_ctx = test_ctx("stacked-and-parallel")?;

        let guard = test_ctx.ctx.project().exclusive_worktree_access();
        let repo = test_ctx.ctx.gix_repo_for_merging()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
        let feature_a = stacks
            .into_iter()
            .flat_map(|s| s.heads)
            .find(|h| h.name == b"feature-a")
            .context("Expect to find feature-a")?;

        let outcome = worktree_new(
            &mut test_ctx.ctx,
            guard.read_permission(),
            feature_a_name.as_ref(),
        )?;

        assert_eq!(
            outcome.created.base,
            Some(feature_a.tip),
            "The base should the the same as the tip of feature-a"
        );
        let worktree = repo.worktrees()?[0].clone();
        let worktree_repo = worktree.clone().into_repo()?;
        assert_eq!(
            worktree.base()?,
            outcome.created.path.canonicalize()?,
            "Worktree should be created where we say"
        );
        assert_eq!(
            Some(worktree_repo.head()?.id().unwrap().detach()),
            outcome.created.base,
            "Worktree should have base checked out"
        );

        Ok(())
    }

    #[test]
    fn can_create_worktree_from_feature_b() -> anyhow::Result<()> {
        let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
        let mut test_ctx = test_ctx("stacked-and-parallel")?;

        let guard = test_ctx.ctx.project().exclusive_worktree_access();
        let repo = test_ctx.ctx.gix_repo_for_merging()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
        let feature_b = stacks
            .into_iter()
            .flat_map(|s| s.heads)
            .find(|h| h.name == b"feature-b")
            .context("Expect to find feature-b")?;

        let outcome = worktree_new(
            &mut test_ctx.ctx,
            guard.read_permission(),
            feature_b_name.as_ref(),
        )?;

        assert_eq!(
            outcome.created.base,
            Some(feature_b.tip),
            "The base should the the same as the tip of feature-b"
        );
        let worktree = repo.worktrees()?[0].clone();
        let worktree_repo = worktree.clone().into_repo()?;
        assert_eq!(
            worktree.base()?,
            outcome.created.path.canonicalize()?,
            "Worktree should be created where we say"
        );
        assert_eq!(
            Some(worktree_repo.head()?.id().unwrap().detach()),
            outcome.created.base,
            "Worktree should have base checked out"
        );

        Ok(())
    }

    #[test]
    fn can_create_worktree_from_feature_c() -> anyhow::Result<()> {
        let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
        let mut test_ctx = test_ctx("stacked-and-parallel")?;

        let guard = test_ctx.ctx.project().exclusive_worktree_access();
        let repo = test_ctx.ctx.gix_repo_for_merging()?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
        let feature_c = stacks
            .into_iter()
            .flat_map(|s| s.heads)
            .find(|h| h.name == b"feature-c")
            .context("Expect to find feature-c")?;

        let outcome = worktree_new(
            &mut test_ctx.ctx,
            guard.read_permission(),
            feature_c_name.as_ref(),
        )?;

        assert_eq!(
            outcome.created.base,
            Some(feature_c.tip),
            "The base should the the same as the tip of feature-c"
        );
        let worktree = repo.worktrees()?[0].clone();
        let worktree_repo = worktree.clone().into_repo()?;
        assert_eq!(
            worktree.base()?,
            outcome.created.path.canonicalize()?,
            "Worktree should be created where we say"
        );
        assert_eq!(
            Some(worktree_repo.head()?.id().unwrap().detach()),
            outcome.created.base,
            "Worktree should have base checked out"
        );

        Ok(())
    }
}

mod worktree_list {
    use super::*;
    use but_worktrees::{list::worktree_list, new::worktree_new};
    use util::test_ctx;

    #[test]
    fn can_list_worktrees() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let guard = ctx.project().exclusive_worktree_access();

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
            dbg!(worktree_list(&mut ctx, guard.read_permission())?)
                .entries
                .iter()
                .all(|e| all.iter().any(|a| a.created == *e))
        );

        Ok(())
    }
}
