mod integrate;

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

    pub trait IntoString {
        fn output_string(&mut self) -> anyhow::Result<String>;
    }

    impl IntoString for std::process::Command {
        fn output_string(&mut self) -> anyhow::Result<String> {
            let output = self.output()?;
            Ok(str::from_utf8(&output.stdout)?.to_owned())
        }
    }
}

mod worktree_new {
    use anyhow::Context;
    use but_graph::VirtualBranchesTomlMetadata;
    use but_workspace::{legacy::StacksFilter, legacy::stacks_v3};
    use but_worktrees::new::worktree_new;
    use util::test_ctx;

    use super::*;

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
    use but_worktrees::{list::worktree_list, new::worktree_new};
    use util::test_ctx;

    use super::*;

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
            worktree_list(&mut ctx, guard.read_permission())?
                .entries
                .iter()
                .all(|e| all.iter().any(|a| a.created == *e))
        );

        Ok(())
    }
}

mod worktree_destroy {
    use but_worktrees::{
        destroy::{worktree_destroy_by_id, worktree_destroy_by_reference},
        list::worktree_list,
        new::worktree_new,
    };
    use util::test_ctx;

    use super::*;

    #[test]
    fn can_destroy_worktree_by_id() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let mut guard = ctx.project().exclusive_worktree_access();

        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let outcome = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

        // Verify it was created
        let list_before = worktree_list(&mut ctx, guard.read_permission())?;
        assert_eq!(list_before.entries.len(), 1);
        assert_eq!(list_before.entries[0].path, outcome.created.path);

        // Destroy it
        let destroy_outcome =
            worktree_destroy_by_id(&mut ctx, guard.write_permission(), &outcome.created.id)?;

        assert_eq!(destroy_outcome.destroyed_ids.len(), 1);
        assert_eq!(destroy_outcome.destroyed_ids[0], outcome.created.id);

        // Verify it was destroyed
        let list_after = worktree_list(&mut ctx, guard.read_permission())?;
        assert_eq!(list_after.entries.len(), 0);

        Ok(())
    }

    #[test]
    fn can_destroy_worktrees_by_reference() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let mut guard = ctx.project().exclusive_worktree_access();

        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;

        // Create 3 worktrees from feature-a and 2 from feature-c
        worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;
        worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;
        worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;

        // Verify all 5 were created
        let list_before = worktree_list(&mut ctx, guard.read_permission())?;
        assert_eq!(list_before.entries.len(), 5);

        // Destroy all feature-a worktrees
        let destroy_outcome = worktree_destroy_by_reference(
            &mut ctx,
            guard.write_permission(),
            feature_a_name.as_ref(),
        )?;

        assert_eq!(destroy_outcome.destroyed_ids.len(), 3);

        // Verify only feature-c worktrees remain
        let list_after = worktree_list(&mut ctx, guard.read_permission())?;
        assert_eq!(list_after.entries.len(), 2);
        assert!(
            list_after
                .entries
                .iter()
                .all(|e| e.created_from_ref.as_ref() == Some(&feature_c_name))
        );

        Ok(())
    }

    #[test]
    fn destroy_by_reference_returns_empty_when_no_matches() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let mut guard = ctx.project().exclusive_worktree_access();

        let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
        let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;

        // Create worktrees from feature-a
        worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

        // Try to destroy worktrees from feature-b (which don't exist)
        let destroy_outcome = worktree_destroy_by_reference(
            &mut ctx,
            guard.write_permission(),
            feature_b_name.as_ref(),
        )?;

        assert_eq!(destroy_outcome.destroyed_ids.len(), 0);

        // Verify feature-a worktree is still there
        let list_after = worktree_list(&mut ctx, guard.read_permission())?;
        assert_eq!(list_after.entries.len(), 1);

        Ok(())
    }
}
