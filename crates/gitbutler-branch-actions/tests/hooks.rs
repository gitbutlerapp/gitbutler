#[cfg(test)]
mod tests {

    use gitbutler_repo::hooks::{ErrorData, HookResult, MessageData, MessageHookResult};
    use gitbutler_testsupport::{Case, Suite};

    #[test]
    fn post_commit_hook_rejection() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
        git2_hooks::create_hook(&*ctx.git2_repo.get()?, git2_hooks::HOOK_POST_COMMIT, hook);

        assert_eq!(
            gitbutler_repo::hooks::post_commit(ctx)?,
            HookResult::Failure(ErrorData {
                error: "rejected\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn message_hook_rejection() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
        git2_hooks::create_hook(&*ctx.git2_repo.get()?, git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Failure(ErrorData {
                error: "rejected\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn rewrite_message() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'rewritten message' > $1
";
        git2_hooks::create_hook(&*ctx.git2_repo.get()?, git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Message(MessageData {
                message: "rewritten message\n".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn keep_message() -> anyhow::Result<()> {
        let suite = Suite::default();
        let Case { ctx, .. } = &suite.new_case();

        let hook = b"
#!/bin/sh
echo 'commit message' > $1
";
        git2_hooks::create_hook(&*ctx.git2_repo.get()?, git2_hooks::HOOK_COMMIT_MSG, hook);

        let message = "commit message\n".to_owned();
        assert_eq!(
            gitbutler_repo::hooks::commit_msg(ctx, message)?,
            MessageHookResult::Success
        );
        Ok(())
    }
}
