use std::borrow::Cow;

use anyhow::{anyhow, Context, Result};
use git2_hooks::HookResult;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Code;

pub fn message(ctx: &CommandContext, mut message: String) -> Result<String> {
    let hook_result = git2_hooks::hooks_commit_msg(ctx.repo(), Some(&["../.husky"]), &mut message)
        .context("failed to run hook")
        .context(Code::CommitHookFailed)?;

    if let HookResult::RunNotSuccessful { stdout, stderr, .. } = &hook_result {
        return Err(
            anyhow!("commit-msg hook rejected: {}", join_output(stdout, stderr))
                .context(Code::CommitHookFailed),
        );
    }
    Ok(message)
}

pub fn pre_commit(ctx: &CommandContext) -> Result<()> {
    let hook_result = git2_hooks::hooks_pre_commit(ctx.repo(), Some(&["../.husky"]))
        .context("failed to run hook")
        .context(Code::CommitHookFailed)?;

    if let HookResult::RunNotSuccessful { stdout, stderr, .. } = &hook_result {
        return Err(
            anyhow!("commit hook rejected: {}", join_output(stdout, stderr))
                .context(Code::CommitHookFailed),
        );
    }
    Ok(())
}

pub fn post_commit(ctx: &CommandContext) -> Result<()> {
    git2_hooks::hooks_post_commit(ctx.repo(), Some(&["../.husky"]))
        .context("failed to run hook")
        .context(Code::CommitHookFailed)?;
    Ok(())
}

fn join_output<'a>(stdout: &'a str, stderr: &'a str) -> Cow<'a, str> {
    let stdout = stdout.trim();
    if stdout.is_empty() {
        stderr.trim().into()
    } else {
        stdout.into()
    }
}
