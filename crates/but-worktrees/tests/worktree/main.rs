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

mod worktree_list {
    use super::*;
    use but_graph::VirtualBranchesTomlMetadata;
    use but_workspace::{StacksFilter, stacks_v3};
    use but_worktrees::{WorktreeHealthStatus, list::worktree_list, new::worktree_new};
    use gitbutler_branch_actions::BranchManagerExt as _;
    use gix::refs::transaction::PreviousValue;
    use util::test_ctx;

    #[test]
    fn can_list_worktrees() -> anyhow::Result<()> {
        let test_ctx = test_ctx("stacked-and-parallel")?;
        let mut ctx = test_ctx.ctx;

        let repo = ctx.gix_repo()?;

        let mut guard = ctx.project().exclusive_worktree_access();

        let a = worktree_new(&mut ctx, guard.read_permission(), "feature-a")?; // To stay Normal
        let b = worktree_new(&mut ctx, guard.read_permission(), "feature-a")?; // To be BranchMissing
        let c = worktree_new(&mut ctx, guard.read_permission(), "feature-a")?; // To be BranchNotCheckedOut
        let d = worktree_new(&mut ctx, guard.read_permission(), "feature-a")?; // To be WorktreeMissing
        let e = worktree_new(&mut ctx, guard.read_permission(), "feature-c")?; // To be WorkspaceBranchMissing

        let all = &[&a, &b, &c, &d, &e];

        // All should start normal
        assert!(
            worktree_list(&mut ctx, guard.read_permission())?
                .entries
                .iter()
                .all(|e| all.iter().any(|a| a.created == e.worktree)
                    && e.status == WorktreeHealthStatus::Normal)
        );

        // remove b's branch
        repo.find_reference(&b.created.reference)?.delete()?;

        // checkout a different branch on c
        repo.reference(
            "refs/heads/new-ref",
            c.created.base,
            PreviousValue::Any,
            "New reference :D",
        )?;
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()))
            .current_dir(&c.created.path)
            .arg("switch")
            .arg("new-ref")
            .output()?;

        // delete d's worktree
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()))
            .current_dir(&ctx.project().path)
            .arg("worktree")
            .arg("remove")
            .arg("-f")
            .arg(d.created.path.as_os_str())
            .output()?;

        // remove `feature-c` branch from workspace
        // It would be nice to invoke the `but` cli here...
        let meta = VirtualBranchesTomlMetadata::from_path(
            ctx.project().gb_dir().join("virtual_branches.toml"),
        )?;
        let stack = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?
            .into_iter()
            .find(|s| s.heads.iter().any(|h| h.name == b"feature-c"))
            .unwrap();
        let branch_manager = ctx.branch_manager();
        branch_manager.unapply(
            stack.id.unwrap(),
            guard.write_permission(),
            false,
            vec![],
            ctx.app_settings().feature_flags.cv3,
        )?;

        assert!(
            worktree_list(&mut ctx, guard.read_permission())?
                .entries
                .into_iter()
                .all(|entry| if entry.worktree == a.created {
                    entry.status == WorktreeHealthStatus::Normal
                } else if entry.worktree == b.created {
                    entry.status == WorktreeHealthStatus::BranchMissing
                } else if entry.worktree == c.created {
                    entry.status == WorktreeHealthStatus::BranchNotCheckedOut
                } else if entry.worktree == d.created {
                    entry.status == WorktreeHealthStatus::WorktreeMissing
                } else if entry.worktree == e.created {
                    entry.status == WorktreeHealthStatus::WorkspaceBranchMissing
                } else {
                    false
                })
        );

        Ok(())
    }
}
