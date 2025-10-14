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

mod stacked_and_parallel {
    use super::*;
    use anyhow::Context;
    use but_graph::VirtualBranchesTomlMetadata;
    use but_workspace::{StacksFilter, stacks_v3};
    use but_worktrees::{WorktreeSource, new::worktree_new};
    use util::test_ctx;

    #[test]
    fn can_create_worktree_from_feature_a() -> anyhow::Result<()> {
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

        let outcome = worktree_new(&mut test_ctx.ctx, guard.read_permission(), "feature-a")?;

        assert_eq!(
            outcome.created.base, feature_a.tip,
            "The base should the the same as the tip of feature-a"
        );
        assert_eq!(
            outcome.created.source,
            WorktreeSource::Branch("feature-a".into()),
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
            worktree_repo.head()?.id().unwrap(),
            outcome.created.base,
            "Worktree should have base checked out"
        );
        assert_eq!(
            *worktree_repo.head()?.referent_name().unwrap().as_bstr(),
            outcome.created.reference,
            "Worktree should have reference checked out"
        );

        Ok(())
    }

    #[test]
    fn can_create_worktree_from_feature_b() -> anyhow::Result<()> {
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

        let outcome = worktree_new(&mut test_ctx.ctx, guard.read_permission(), "feature-b")?;

        assert_eq!(
            outcome.created.base, feature_b.tip,
            "The base should the the same as the tip of feature-b"
        );
        assert_eq!(
            outcome.created.source,
            WorktreeSource::Branch("feature-b".into()),
            "The source should be feature-b"
        );
        let worktree = repo.worktrees()?[0].clone();
        let worktree_repo = worktree.clone().into_repo()?;
        assert_eq!(
            worktree.base()?,
            outcome.created.path.canonicalize()?,
            "Worktree should be created where we say"
        );
        assert_eq!(
            worktree_repo.head()?.id().unwrap(),
            outcome.created.base,
            "Worktree should have base checked out"
        );
        assert_eq!(
            *worktree_repo.head()?.referent_name().unwrap().as_bstr(),
            outcome.created.reference,
            "Worktree should have reference checked out"
        );

        Ok(())
    }

    #[test]
    fn can_create_worktree_from_feature_c() -> anyhow::Result<()> {
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

        let outcome = worktree_new(&mut test_ctx.ctx, guard.read_permission(), "feature-c")?;

        assert_eq!(
            outcome.created.base, feature_c.tip,
            "The base should the the same as the tip of feature-c"
        );
        assert_eq!(
            outcome.created.source,
            WorktreeSource::Branch("feature-c".into()),
            "The source should be feature-c"
        );
        let worktree = repo.worktrees()?[0].clone();
        let worktree_repo = worktree.clone().into_repo()?;
        assert_eq!(
            worktree.base()?,
            outcome.created.path.canonicalize()?,
            "Worktree should be created where we say"
        );
        assert_eq!(
            worktree_repo.head()?.id().unwrap(),
            outcome.created.base,
            "Worktree should have base checked out"
        );
        assert_eq!(
            *worktree_repo.head()?.referent_name().unwrap().as_bstr(),
            outcome.created.reference,
            "Worktree should have reference checked out"
        );

        Ok(())
    }
}
