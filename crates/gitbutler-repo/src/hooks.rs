use crate::staging;
use anyhow::Result;
use bstr::ByteSlice;
use git2_hooks;
use git2_hooks::HookResult as H;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::GitHunk;
use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

#[derive(Serialize, PartialEq, Debug, Clone)]
pub struct MessageData {
    pub message: String,
}

#[derive(Serialize, PartialEq, Debug, Clone)]
pub struct ErrorData {
    pub error: String,
}

/// Hook result indicating either success or failure.
#[derive(Serialize, PartialEq, Debug, Clone)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum HookResult {
    Success,
    NotConfigured,
    Failure(ErrorData),
}

/// Message hook result indicating either success, message, or failure.
///
/// A message hook can optionally mutate the message, so this special type is
/// needed to distinguish between success, and success with message.
#[derive(Serialize, PartialEq, Debug, Clone)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum MessageHookResult {
    Success,
    NotConfigured,
    Message(MessageData),
    Failure(ErrorData),
}

pub fn commit_msg(ctx: &CommandContext, mut message: String) -> Result<MessageHookResult> {
    let original_message = message.clone();
    match git2_hooks::hooks_commit_msg(ctx.repo(), Some(&["../.husky"]), &mut message)? {
        H::Ok { hook: _ } => match message == original_message {
            true => Ok(MessageHookResult::Success),
            false => Ok(MessageHookResult::Message(MessageData { message })),
        },
        H::NoHookFound => Ok(MessageHookResult::NotConfigured),
        H::RunNotSuccessful {
            stdout,
            stderr,
            code,
            ..
        } => {
            let error = join_output(stdout, stderr, code);
            Ok(MessageHookResult::Failure(ErrorData { error }))
        }
    }
}

pub fn pre_commit(
    ctx: &CommandContext,
    selected_hunks: &[(PathBuf, Vec<GitHunk>)],
) -> Result<HookResult> {
    let repo = ctx.repo();
    let original_tree = repo.index()?.write_tree()?;

    // Scope guard that resets the index at the end, even under panic.
    let _guard = scopeguard::guard((), |_| {
        match staging::reset_index(repo, original_tree) {
            Ok(()) => (),
            Err(err) => tracing::error!("Failed to reset index: {}", err),
        };
    });

    staging::stage(ctx, selected_hunks)?;
    Ok(
        match git2_hooks::hooks_pre_commit(ctx.repo(), Some(&["../.husky"]))? {
            H::Ok { hook: _ } => HookResult::Success,
            H::NoHookFound => HookResult::NotConfigured,
            H::RunNotSuccessful {
                stdout,
                stderr,
                code,
                ..
            } => {
                let error = join_output(stdout, stderr, code);
                HookResult::Failure(ErrorData { error })
            }
        },
    )
}

pub fn pre_commit_with_tree(ctx: &CommandContext, tree_id: git2::Oid) -> Result<HookResult> {
    let repo = ctx.repo();
    let original_tree = repo.index()?.write_tree()?;

    // Scope guard that resets the index at the end, even under panic.
    let _guard = scopeguard::guard((), |_| {
        match staging::reset_index(repo, original_tree) {
            Ok(()) => (),
            Err(err) => tracing::error!("Failed to reset index: {}", err),
        };
    });

    let mut index = repo.index()?;
    index.read_tree(&repo.find_tree(tree_id)?)?;
    index.write()?;

    Ok(
        match git2_hooks::hooks_pre_commit(ctx.repo(), Some(&["../.husky"]))? {
            H::Ok { hook: _ } => HookResult::Success,
            H::NoHookFound => HookResult::NotConfigured,
            H::RunNotSuccessful {
                stdout,
                stderr,
                code,
                ..
            } => {
                let error = join_output(stdout, stderr, code);
                HookResult::Failure(ErrorData { error })
            }
        },
    )
}

pub fn post_commit(ctx: &CommandContext) -> Result<HookResult> {
    match git2_hooks::hooks_post_commit(ctx.repo(), Some(&["../.husky"]))? {
        H::Ok { hook: _ } => Ok(HookResult::Success),
        H::NoHookFound => Ok(HookResult::NotConfigured),
        H::RunNotSuccessful {
            stdout,
            stderr,
            code,
            ..
        } => {
            let error = join_output(stdout, stderr, code);
            Ok(HookResult::Failure(ErrorData { error }))
        }
    }
}

// TODO: double-check this with what should happen according to Git; contribute to `git2-hooks` possibly.
/// Since git2-hooks doesn't support pre-push yet, we implement it ourselves
/// following the same pattern as the existing hooks
/// Use `oid` and `remote_tracking_branch` to deduce the refspec information. Note that this isn't general, but should
/// work for us.
pub fn pre_push(
    repo: &git2::Repository,
    remote_name: &str,
    remote_url: &str,
    local_commit: git2::Oid,
    remote_tracking_branch: &gitbutler_reference::RemoteRefname,
) -> Result<HookResult> {
    let hooks_path = repo.path().join("hooks").join("pre-push");
    let husky_path = repo
        .path()
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join(".husky").join("pre-push"));

    // Check for hook in .git/hooks/pre-push first, then ../.husky/pre-push
    let hook_path = hooks_path
        .exists()
        .then_some(hooks_path)
        .or_else(|| husky_path.filter(|path| path.exists()));

    let Some(hook_path) = hook_path else {
        return Ok(HookResult::NotConfigured);
    };

    // Execute the pre-push hook with remote name and URL as arguments
    let mut child = std::process::Command::from({
        let mut prep = gix::command::prepare(&hook_path);
        if cfg!(windows) {
            prep.use_shell = true;
            prep.allow_manual_arg_splitting = false;
            // Need unix separators for the unix bash to not swallow the backslash!
            let with_slashes_for_bash = gix::path::to_unix_separators_on_windows(
                gix::path::os_str_into_bstr(&prep.command)?,
            );
            prep.command = gix::path::from_bstring(with_slashes_for_bash.into_owned()).into();
        }
        prep.arg(remote_name).arg(remote_url)
    })
    .current_dir(repo.workdir().unwrap_or_else(|| repo.path()))
    .stdin(Stdio::piped())
    .spawn()?;

    {
        let remote_commit = repo
            .find_reference(&remote_tracking_branch.to_string())
            .ok()
            .and_then(|r| r.target())
            .unwrap_or_else(git2::Oid::zero);
        // THIS IS WRONG: but is correct the common case. This also is an issue when the ref is actually pushed,
        // but we can fix it when moving everything to `gix`.
        let local_tracking_branch_deduced =
            format!("refs/heads/{}", remote_tracking_branch.branch());
        let stdin = child.stdin.as_mut().expect("configured");
        stdin.write_all(
            format!("{local_tracking_branch_deduced} {local_commit} {remote_tracking_branch} {remote_commit}\n")
                .as_bytes(),
        )?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(HookResult::Success)
    } else {
        let error = join_output(
            output.stdout.to_str_lossy().into_owned(),
            output.stderr.to_str_lossy().into_owned(),
            output.status.code(),
        );
        Ok(HookResult::Failure(ErrorData { error }))
    }
}

fn join_output(stdout: String, stderr: String, code: Option<i32>) -> String {
    let code = code
        .map(|code| format!(" (Exit Code {code})"))
        .unwrap_or_default();
    if stdout.is_empty() && stderr.is_ascii() {
        return format!("hook produced no output{code}");
    } else if stdout.is_empty() {
        return stderr;
    } else if stderr.is_empty() {
        return stdout;
    }
    format!("stdout:\n{stdout}\n\nstderr:\n{stderr}{code}")
}
