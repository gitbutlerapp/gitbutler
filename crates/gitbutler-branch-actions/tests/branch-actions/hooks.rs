#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;
use std::{fs, path::PathBuf};

use gitbutler_repo::hooks::{ErrorData, HookResult, MessageData, MessageHookResult};

use crate::support::hook_case;

fn write_hook(ctx: &but_ctx::Context, name: &str, hook: &[u8]) -> anyhow::Result<PathBuf> {
    let repo = ctx.repo.get()?;
    let hook_path = repo.git_dir().join("hooks").join(name);
    fs::create_dir_all(hook_path.parent().expect("hook path has parent"))?;
    fs::write(&hook_path, hook)?;
    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    Ok(hook_path)
}

#[cfg(unix)]
#[test]
fn post_commit_hook_inherits_application_path() -> anyhow::Result<()> {
    const CHILD_MARKER: &str = "GITBUTLER_HOOK_PATH_TEST_CHILD";
    const OUTPUT: &str = "GITBUTLER_HOOK_PATH_TEST_OUTPUT";

    if std::env::var_os(CHILD_MARKER).is_none() {
        let case = hook_case()?;
        let workdir = case
            .ctx
            .repo
            .get()?
            .workdir()
            .expect("non-bare worktree")
            .to_owned();
        let bin_dir = workdir.join("login-shell-bin");
        let tool_path = bin_dir.join("gitbutler-hook-path-tool");
        let output_path = workdir.join("hook-path-output");
        fs::create_dir_all(&bin_dir)?;
        fs::write(
            &tool_path,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$GITBUTLER_HOOK_PATH_TEST_OUTPUT\"\n",
        )?;
        fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o755))?;

        let path = std::env::join_paths(std::iter::once(bin_dir).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))?;
        let result = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "hooks::post_commit_hook_inherits_application_path",
            ])
            .env(CHILD_MARKER, "1")
            .env(OUTPUT, &output_path)
            .env("PATH", path)
            .output()?;

        assert!(
            result.status.success(),
            "child hook test failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read_to_string(output_path)?, "post-commit\n");
        return Ok(());
    }

    let mut case = hook_case()?;
    case.ctx.legacy_project.husky_hooks_enabled = true;
    write_hook(
        &case.ctx,
        "post-commit",
        b"#!/bin/sh\ngitbutler-hook-path-tool post-commit\n",
    )?;

    assert_eq!(
        gitbutler_repo::hooks::post_commit(&case.ctx)?,
        HookResult::Success
    );
    Ok(())
}

#[test]
fn post_commit_hook_rejection() -> anyhow::Result<()> {
    let mut case = hook_case()?;
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let ctx = &case.ctx;

    let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
    write_hook(ctx, "post-commit", hook)?;

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
    let mut case = hook_case()?;
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let ctx = &case.ctx;

    let hook = b"
#!/bin/sh
echo 'rejected'
exit 1
";
    write_hook(ctx, "commit-msg", hook)?;

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
    let mut case = hook_case()?;
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let ctx = &case.ctx;

    let hook = b"
#!/bin/sh
echo 'rewritten message' > $1
";
    write_hook(ctx, "commit-msg", hook)?;

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
    let mut case = hook_case()?;
    case.ctx.legacy_project.husky_hooks_enabled = true;
    let ctx = &case.ctx;

    let hook = b"
#!/bin/sh
echo 'commit message' > $1
";
    write_hook(ctx, "commit-msg", hook)?;

    let message = "commit message\n".to_owned();
    assert_eq!(
        gitbutler_repo::hooks::commit_msg(ctx, message)?,
        MessageHookResult::Success
    );
    Ok(())
}

#[test]
fn husky_hooks_disabled_even_if_present() -> anyhow::Result<()> {
    let case = hook_case()?;
    let ctx = &case.ctx;

    let repo = ctx.repo.get()?;
    let husky_dir = repo.workdir().expect("non-bare worktree").join(".husky");
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
