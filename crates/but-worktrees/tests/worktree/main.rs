/// Tests for worktree creation and management
mod util {
    use but_ctx::Context;
    use but_testsupport::gix_testtools::tempfile::TempDir;
    use but_worktrees::{
        WorktreeId,
        destroy::DestroyWorktreeOutcome,
        integrate::{WorktreeIntegrationStatus, worktree_integrate, worktree_integration_status},
        list::ListWorktreeOutcome,
        new::NewWorktreeOutcome,
    };

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (repo, tmpdir) = but_testsupport::writable_scenario(name);
        let ctx = Context::from_repo(repo)?;

        Ok(TestContext { ctx, tmpdir })
    }

    pub struct TestContext {
        pub ctx: Context,
        #[expect(unused)]
        pub tmpdir: TempDir,
    }

    /// Derive the narrow inputs `worktree_new` needs from `ctx`.
    pub fn worktree_new(
        ctx: &Context,
        perm: &but_ctx::access::RepoShared,
        refname: &gix::refs::FullNameRef,
    ) -> anyhow::Result<NewWorktreeOutcome> {
        let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
        but_worktrees::new::worktree_new(&repo, &ws, &ctx.project_data_dir(), refname)
    }

    pub fn worktree_list(ctx: &Context) -> anyhow::Result<ListWorktreeOutcome> {
        let repo = ctx.repo.get()?;
        but_worktrees::list::worktree_list(&repo)
    }

    pub fn worktree_destroy_by_id(
        ctx: &Context,
        id: &WorktreeId,
    ) -> anyhow::Result<DestroyWorktreeOutcome> {
        let repo = ctx.repo.get()?;
        but_worktrees::destroy::worktree_destroy_by_id(&repo, id)
    }

    pub fn worktree_destroy_by_reference(
        ctx: &Context,
        reference: &gix::refs::FullNameRef,
    ) -> anyhow::Result<DestroyWorktreeOutcome> {
        let repo = ctx.repo.get()?;
        but_worktrees::destroy::worktree_destroy_by_reference(&repo, reference)
    }

    /// Derive the narrow inputs the integration functions need from `ctx`.
    pub fn integration_status(
        ctx: &Context,
        perm: &but_ctx::access::RepoExclusive,
        id: &WorktreeId,
        target: &gix::refs::FullNameRef,
    ) -> anyhow::Result<WorktreeIntegrationStatus> {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
        worktree_integration_status(&repo, &mut ws, &mut meta, id, target)
    }

    /// Derive the narrow inputs the integration functions need from `ctx`.
    pub fn integrate(
        ctx: &Context,
        perm: &but_ctx::access::RepoExclusive,
        id: &WorktreeId,
        target: &gix::refs::FullNameRef,
    ) -> anyhow::Result<()> {
        let mut meta = ctx.meta()?;
        let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
        worktree_integrate(&repo, &mut ws, &mut meta, id, target)
    }
}

mod worktree_new;

mod worktree_list {
    use crate::util::{test_ctx, worktree_list, worktree_new};

    #[test]
    fn can_list_worktrees() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let guard = ctx.exclusive_worktree_access();

        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
        let a = worktree_new(&ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let b = worktree_new(&ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let c = worktree_new(&ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let d = worktree_new(&ctx, guard.read_permission(), feature_a_name.as_ref())?;
        let e = worktree_new(&ctx, guard.read_permission(), feature_c_name.as_ref())?;

        let all = &[&a, &b, &c, &d, &e];

        // All should start normal
        assert!(
            worktree_list(&ctx)?
                .entries
                .iter()
                .all(|e| all.iter().any(|a| a.created == *e))
        );

        Ok(())
    }
}

mod various;
mod worktree_destroy;
