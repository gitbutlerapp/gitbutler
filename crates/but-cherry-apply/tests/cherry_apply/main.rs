/// Tests for cherry-apply functionality
mod util {
    use but_cherry_apply::{CherryApplyStatus, cherry_apply, cherry_apply_status};
    use but_ctx::Context;
    use but_testsupport::gix_testtools::tempfile::TempDir;
    use gitbutler_stack::VirtualBranchesHandle;

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (repo, tmpdir) = but_testsupport::writable_scenario(name);
        // TODO: all this should work without `Context` once it's switched to the new rebase engine,
        //       making this crate either obsolete or proper plumbing.
        let ctx = Context::from_repo(repo)?;
        // update the vb-toml metadata - trigger reconciliation and write the vb.toml according to what's there.
        {
            let guard = ctx.shared_worktree_access();
            let meta = ctx.legacy_meta(guard.read_permission())?;
            meta.write_reconciled(&*ctx.repo.get()?)?;
        }
        let handle = VirtualBranchesHandle::new(ctx.project_data_dir());

        Ok(TestContext {
            ctx,
            handle,
            _tmpdir: tmpdir,
        })
    }

    pub struct TestContext {
        pub ctx: Context,
        pub handle: VirtualBranchesHandle,
        pub _tmpdir: TempDir,
    }

    impl TestContext {
        pub fn get_status(&self, commit_id: gix::ObjectId) -> anyhow::Result<CherryApplyStatus> {
            cherry_apply_status(
                &self.ctx,
                self.ctx.exclusive_worktree_access().read_permission(),
                commit_id,
            )
        }

        pub fn apply(
            &self,
            commit_id: gix::ObjectId,
            target_stack: but_core::ref_metadata::StackId,
        ) -> anyhow::Result<()> {
            cherry_apply(
                &self.ctx,
                self.ctx.exclusive_worktree_access().write_permission(),
                commit_id,
                target_stack,
            )
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
        let test_ctx = test_ctx("no-stacks")?;

        let repo = test_ctx.ctx.repo.get()?;
        let commit_id = repo
            .rev_parse_single("refs/gitbutler/no-stacks-commit")?
            .detach();

        let status = test_ctx.get_status(commit_id)?;

        assert_eq!(status, CherryApplyStatus::NoStacks);

        Ok(())
    }
}
