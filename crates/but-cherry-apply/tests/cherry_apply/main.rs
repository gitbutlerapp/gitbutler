/// Tests for cherry-apply functionality
mod util {
    use but_cherry_apply::{CherryApplyStatus, cherry_apply, cherry_apply_status};
    use gitbutler_command_context::CommandContext;
    use gitbutler_stack::VirtualBranchesHandle;
    use gix_testtools::tempfile::TempDir;

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (ctx, tmpdir) = gitbutler_testsupport::writable::fixture("cherry_apply.sh", name)?;
        let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());

        Ok(TestContext {
            ctx,
            handle,
            _tmpdir: tmpdir,
        })
    }

    pub struct TestContext {
        pub ctx: CommandContext,
        pub handle: VirtualBranchesHandle,
        pub _tmpdir: TempDir,
    }

    impl TestContext {
        pub fn get_status(&self, commit_id: gix::ObjectId) -> anyhow::Result<CherryApplyStatus> {
            cherry_apply_status(
                &self.ctx,
                self.ctx
                    .project()
                    .exclusive_worktree_access()
                    .read_permission(),
                commit_id,
            )
        }

        pub fn apply(
            &self,
            commit_id: gix::ObjectId,
            target_stack: but_workspace::StackId,
        ) -> anyhow::Result<()> {
            cherry_apply(
                &self.ctx,
                self.ctx
                    .project()
                    .exclusive_worktree_access()
                    .write_permission(),
                commit_id,
                target_stack,
            )
        }
    }
}

use but_cherry_apply::CherryApplyStatus;
use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::legacy::stack_details_v3;

mod clean_to_both {
    use util::test_ctx;

    use super::*;

    #[test]
    fn status_is_applicable_to_any_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("clean-to-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/clean-commit")?
            .detach();

        let status = test_ctx.get_status(commit_id)?;

        assert_eq!(status, CherryApplyStatus::ApplicableToAnyStack);

        Ok(())
    }

    #[test]
    fn can_apply_to_foo_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("clean-to-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/clean-commit")?
            .detach();

        let foo_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "foo")
            .unwrap()
            .id;

        // Apply should succeed
        test_ctx.apply(commit_id, foo_id)?;

        // Verify the commit is now in the foo stack by checking for its message
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let details = stack_details_v3(Some(foo_id), &repo, &meta)?;

        let has_commit = details
            .branch_details
            .iter()
            .flat_map(|branch| &branch.commits)
            .any(|commit| {
                commit
                    .message
                    .to_string()
                    .contains("Add clean change to shared.txt")
            });

        assert!(
            has_commit,
            "Expected to find cherry-picked commit in foo stack"
        );

        Ok(())
    }

    #[test]
    fn can_apply_to_bar_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("clean-to-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/clean-commit")?
            .detach();

        let bar_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "bar")
            .unwrap()
            .id;

        // Apply should succeed
        test_ctx.apply(commit_id, bar_id)?;

        // Verify the commit is now in the bar stack by checking for its message
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let details = stack_details_v3(Some(bar_id), &repo, &meta)?;

        let has_commit = details
            .branch_details
            .iter()
            .flat_map(|branch| &branch.commits)
            .any(|commit| {
                commit
                    .message
                    .to_string()
                    .contains("Add clean change to shared.txt")
            });

        assert!(
            has_commit,
            "Expected to find cherry-picked commit in bar stack"
        );

        Ok(())
    }
}

mod conflicts_with_bar {
    use util::test_ctx;

    use super::*;

    #[test]
    fn status_is_locked_to_bar() -> anyhow::Result<()> {
        let test_ctx = test_ctx("conflicts-with-bar")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/bar-conflict")?
            .detach();

        let status = test_ctx.get_status(commit_id)?;

        let bar_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "bar")
            .unwrap()
            .id;

        assert_eq!(status, CherryApplyStatus::LockedToStack(bar_id));

        Ok(())
    }

    #[test]
    fn can_only_apply_to_bar_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("conflicts-with-bar")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/bar-conflict")?
            .detach();

        let bar_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "bar")
            .unwrap()
            .id;

        // Apply to bar should succeed
        test_ctx.apply(commit_id, bar_id)?;

        // Verify the commit is now in the bar stack by checking for its message
        let meta = VirtualBranchesTomlMetadata::from_path(
            test_ctx
                .ctx
                .project()
                .gb_dir()
                .join("virtual_branches.toml"),
        )?;
        let details = stack_details_v3(Some(bar_id), &repo, &meta)?;

        let has_commit = details
            .branch_details
            .iter()
            .flat_map(|branch| &branch.commits)
            .any(|commit| {
                commit
                    .message
                    .to_string()
                    .contains("Conflicting change to bar.txt")
            });

        assert!(
            has_commit,
            "Expected to find cherry-picked commit in bar stack"
        );

        Ok(())
    }
}

mod conflicts_with_both {
    use util::test_ctx;

    use super::*;

    #[test]
    fn status_is_causes_workspace_conflict() -> anyhow::Result<()> {
        let test_ctx = test_ctx("conflicts-with-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/both-conflict")?
            .detach();

        let status = test_ctx.get_status(commit_id)?;

        assert_eq!(status, CherryApplyStatus::CausesWorkspaceConflict);

        Ok(())
    }

    #[test]
    fn cannot_apply_to_foo_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("conflicts-with-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/both-conflict")?
            .detach();

        let foo_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "foo")
            .unwrap()
            .id;

        // Apply should fail
        let result = test_ctx.apply(commit_id, foo_id);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("causes workspace conflicts")
        );

        Ok(())
    }

    #[test]
    fn cannot_apply_to_bar_stack() -> anyhow::Result<()> {
        let test_ctx = test_ctx("conflicts-with-both")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/both-conflict")?
            .detach();

        let bar_id = test_ctx
            .handle
            .list_stacks_in_workspace()?
            .iter()
            .find(|s| s.name == "bar")
            .unwrap()
            .id;

        // Apply should fail
        let result = test_ctx.apply(commit_id, bar_id);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("causes workspace conflicts")
        );

        Ok(())
    }
}

mod no_stacks {
    use util::test_ctx;

    use super::*;

    #[test]
    fn status_is_no_stacks() -> anyhow::Result<()> {
        let test_ctx = test_ctx("no-stacks")?;

        let repo = test_ctx.ctx.gix_repo()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/no-stacks-commit")?
            .detach();

        let status = test_ctx.get_status(commit_id)?;

        assert_eq!(status, CherryApplyStatus::NoStacks);

        Ok(())
    }
}
