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

fn join_output(stdout: String, stderr: String) -> String {
    if stdout.is_empty() && stderr.is_ascii() {
        return "hook produced no output".to_owned();
    } else if stdout.is_empty() {
        return stderr;
    } else if stderr.is_empty() {
        return stdout;
    }
    format!("stdout:\n{}\n\nstderr:\n{}", stdout, stderr)
}
