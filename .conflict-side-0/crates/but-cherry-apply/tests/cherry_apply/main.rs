#![expect(
    deprecated,
    reason = "imports but_workspace::legacy::stack_details_v3; this should be replaced with ctx.workspace_* helpers"
)]

/// Tests for cherry-apply functionality
mod util {
    use anyhow::Context as _;
    use but_cherry_apply::{CherryApplyStatus, cherry_apply, cherry_apply_status};
    use but_core::{
        RefMetadata as _, WORKSPACE_REF_NAME,
        ref_metadata::{StackId, StackKind},
    };
    use but_ctx::Context;
    use but_testsupport::gix_testtools::tempfile::TempDir;

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
        Ok(TestContext {
            ctx,
            _tmpdir: tmpdir,
        })
    }

    pub struct TestContext {
        pub ctx: Context,
        pub _tmpdir: TempDir,
    }

    impl TestContext {
        pub fn stack_id(&self, name: &str) -> anyhow::Result<StackId> {
            let meta = self.ctx.legacy_meta()?;
            let workspace_ref = WORKSPACE_REF_NAME.try_into()?;
            meta.workspace(workspace_ref)?
                .stacks(StackKind::Applied)
                .find(|stack| {
                    stack
                        .name()
                        .is_some_and(|stack_name| stack_name.shorten() == name)
                })
                .map(|stack| stack.id)
                .with_context(|| format!("Expected to find stack named {name}"))
        }

        pub fn get_status(
            &mut self,
            commit_id: gix::ObjectId,
        ) -> anyhow::Result<CherryApplyStatus> {
            let guard = self.ctx.exclusive_worktree_access();
            cherry_apply_status(&self.ctx, guard.read_permission(), commit_id)
        }

        pub fn apply(
            &mut self,
            commit_id: gix::ObjectId,
            target_stack: but_core::ref_metadata::StackId,
        ) -> anyhow::Result<()> {
            let mut guard = self.ctx.exclusive_worktree_access();
            cherry_apply(&self.ctx, guard.write_permission(), commit_id, target_stack)
        }
    }
}

use but_cherry_apply::CherryApplyStatus;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::legacy::stack_details_v3;

mod clean_to_both;
mod conflicts_with_bar;
mod conflicts_with_both;

mod no_stacks {
    use super::*;
    use crate::util::test_ctx;

    #[test]
    fn status_is_no_stacks() -> anyhow::Result<()> {
        let mut test_ctx = test_ctx("no-stacks")?;

        let repo = test_ctx.ctx.repo.get()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/no-stacks-commit")?
            .detach();

        drop(repo);
        let status = test_ctx.get_status(commit_id)?;

        assert_eq!(status, CherryApplyStatus::NoStacks);

        Ok(())
    }
}
