mod util {
    use gitbutler_command_context::CommandContext;
    use gitbutler_stack::VirtualBranchesHandle;
    use gix_testtools::tempfile::TempDir;

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        let (ctx, tmpdir) = gitbutler_testsupport::writable::fixture("example.sh", name)?;
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

mod example_scenario {
    use super::*;
    use util::test_ctx;

    #[test]
    fn test_example_scenario() -> anyhow::Result<()> {
        let test_ctx = test_ctx("example-scenario")?;
        let stacks = test_ctx.handle.list_stacks_in_workspace()?;
        assert!(!stacks.is_empty());
        Ok(())
    }
}
