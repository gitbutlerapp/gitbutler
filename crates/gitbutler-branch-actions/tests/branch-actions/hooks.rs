use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use but_testsupport::legacy::{Case, Suite};
use gitbutler_repo::hooks::{ErrorData, HookResult, MessageData, MessageHookResult};

#[test]
fn post_commit_hook_rejection() -> anyhow::Result<()> {
    let suite = Suite::default();
    let mut case = suite.new_case();
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let Case { ctx, .. } = &case;

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
    let mut case = suite.new_case();
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let Case { ctx, .. } = &case;

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
    let mut case = suite.new_case();
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let Case { ctx, .. } = &case;

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
    let mut case = suite.new_case();
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let Case { ctx, .. } = &case;

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

#[test]
fn husky_hooks_disabled_even_if_present() -> anyhow::Result<()> {
    let suite = Suite::default();
    let case = suite.new_case();
    let Case { ctx, .. } = &case;

    let repo = ctx.git2_repo.get()?;
    let husky_dir = repo
        .path()
        .parent()
        .expect("git dir in worktree")
        .join(".husky");
    fs::create_dir_all(&husky_dir)?;

    let post_hook_path = husky_dir.join("post-commit");
    let msg_hook_path = husky_dir.join("commit-msg");
    fs::write(
        &post_hook_path,
        "#!/bin/sh\necho 'should not run'\nexit 1\n",
    )?;
    fs::write(&msg_hook_path, "#!/bin/sh\necho 'should not run'\nexit 1\n")?;

    #[cfg(unix)]
    {
        fs::set_permissions(&post_hook_path, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&msg_hook_path, fs::Permissions::from_mode(0o755))?;
    }

    assert_eq!(
        gitbutler_repo::hooks::post_commit(ctx)?,
        HookResult::NotConfigured
    );
    assert_eq!(
        gitbutler_repo::hooks::commit_msg(ctx, "commit message".to_owned())?,
        MessageHookResult::NotConfigured
    );
    Ok(())
}
