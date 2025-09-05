use std::path::PathBuf;

use anyhow::Result;
use git2_hooks;
use git2_hooks::HookResult as H;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::GitHunk;
use serde::Serialize;

use crate::staging;

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
        H::RunNotSuccessful { stdout, stderr, .. } => {
            let error = join_output(stdout, stderr);
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
            H::RunNotSuccessful { stdout, stderr, .. } => {
                let error = join_output(stdout, stderr);
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
            H::RunNotSuccessful { stdout, stderr, .. } => {
                let error = join_output(stdout, stderr);
                HookResult::Failure(ErrorData { error })
            }
        },
    )
}

pub fn post_commit(ctx: &CommandContext) -> Result<HookResult> {
    match git2_hooks::hooks_post_commit(ctx.repo(), Some(&["../.husky"]))? {
        H::Ok { hook: _ } => Ok(HookResult::Success),
        H::NoHookFound => Ok(HookResult::NotConfigured),
        H::RunNotSuccessful { stdout, stderr, .. } => {
            let error = join_output(stdout, stderr);
            Ok(HookResult::Failure(ErrorData { error }))
        }
    }
}

pub fn pre_push(
    ctx: &CommandContext,
    remote_name: &str,
    remote_url: &str,
) -> Result<HookResult> {
    // Implement pre-push hook following the same pattern as git2-hooks
    // Pre-push hooks receive the remote name and URL as parameters
    use std::process::Command;
    
    let repo = ctx.repo();
    
    // Look for pre-push hook in .git/hooks/ and .husky/
    let git_dir = repo.path();
    let hooks_dir = git_dir.join("hooks");
    let hook_path = hooks_dir.join("pre-push");
    
    // Also check .husky directory (relative to repo root)
    let repo_root = git_dir.parent().unwrap_or(git_dir);
    let husky_hook_path = repo_root.join(".husky").join("pre-push");
    
    let hook_to_run = if hook_path.exists() && hook_path.is_file() {
        Some(hook_path)
    } else if husky_hook_path.exists() && husky_hook_path.is_file() {
        Some(husky_hook_path)
    } else {
        None
    };
    
    let Some(hook_path) = hook_to_run else {
        return Ok(HookResult::NotConfigured);
    };
    
    // Pre-push hooks receive remote name and URL as environment variables
    // and on stdin receive lines like: <local ref> <local sha1> <remote ref> <remote sha1>
    let mut cmd = Command::new(&hook_path);
    cmd.current_dir(repo_root);
    cmd.env("GIT_DIR", git_dir);
    cmd.arg(remote_name);
    cmd.arg(remote_url);
    
    let output = cmd.output().map_err(|e| anyhow::anyhow!("Failed to execute pre-push hook: {}", e))?;
    
    if output.status.success() {
        Ok(HookResult::Success)
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let error = join_output(stdout, stderr);
        Ok(HookResult::Failure(ErrorData { error }))
    }
}

fn join_output(stdout: String, stderr: String) -> String {
    if stdout.is_empty() && stderr.is_ascii() {
        return "hook produced no output".to_owned();
    } else if stdout.is_empty() {
        return stderr;
    } else if stderr.is_empty() {
        return stdout;
    }
    format!("stdout:\n{stdout}\n\nstderr:\n{stderr}")
}
