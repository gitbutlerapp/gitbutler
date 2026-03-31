use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use git2_hooks::{self, HookResult as H};
use serde::Serialize;

use crate::{managed_hooks::get_hooks_dir_gix, staging};

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

fn husky_search_paths(ctx: &Context) -> Option<&'static [&'static str]> {
    if ctx.legacy_project.husky_hooks_enabled {
        Some(&["../.husky"])
    } else {
        None
    }
}

pub fn commit_msg(ctx: &Context, mut message: String) -> Result<MessageHookResult> {
    let original_message = message.clone();
    match git2_hooks::hooks_commit_msg(
        &*ctx.git2_repo.get()?,
        husky_search_paths(ctx),
        &mut message,
    )? {
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

pub fn pre_commit_with_tree(ctx: &Context, tree_id: gix::ObjectId) -> Result<HookResult> {
    let repo = &*ctx.git2_repo.get()?;
    let original_tree = repo.index()?.write_tree()?;

    // Scope guard that resets the index at the end, even under panic.
    let _guard = scopeguard::guard((), |_| {
        match staging::reset_index(repo, original_tree) {
            Ok(()) => (),
            Err(err) => tracing::error!("Failed to reset index: {}", err),
        };
    });

    let mut index = repo.index()?;
    index.read_tree(&repo.find_tree(tree_id.to_git2())?)?;
    index.write()?;

    Ok(
        match git2_hooks::hooks_pre_commit(&*ctx.git2_repo.get()?, husky_search_paths(ctx))? {
            H::Ok { hook: _ } => HookResult::Success,
            H::NoHookFound => HookResult::NotConfigured,
            H::RunNotSuccessful {
                stdout,
                stderr,
                code,
                ..
            } => {
                // If the output contains GITBUTLER_ERROR, it's our managed hook blocking
                // commits on gitbutler/workspace - this is expected behavior, not a failure
                if stdout.contains("GITBUTLER_ERROR") || stderr.contains("GITBUTLER_ERROR") {
                    HookResult::Success
                } else {
                    let error = join_output(stdout, stderr, code);
                    HookResult::Failure(ErrorData { error })
                }
            }
        },
    )
}

pub fn post_commit(ctx: &Context) -> Result<HookResult> {
    match git2_hooks::hooks_post_commit(&*ctx.git2_repo.get()?, husky_search_paths(ctx))? {
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
/// Use `local_commit` and `remote_tracking_branch` to deduce the refspec information. Note that
/// this isn't general, but should work for us.
pub fn pre_push(
    repo: &gix::Repository,
    remote_name: &str,
    remote_url: &str,
    local_commit: gix::ObjectId,
    remote_tracking_branch: &gitbutler_reference::RemoteRefname,
    run_husky_hooks: bool,
) -> Result<HookResult> {
    let hooks_dir = get_hooks_dir_gix(repo);
    let hooks_path = hooks_dir.join("pre-push");
    let husky_path = run_husky_hooks.then(|| {
        repo.workdir()
            .map(|workdir| workdir.join(".husky").join("pre-push"))
    });

    // Check for hook in .git/hooks/pre-push first, then ../.husky/pre-push
    let hook_path = hooks_path
        .exists()
        .then_some(hooks_path)
        .filter(|path| run_husky_hooks || !path_is_in_husky_dir(repo, path))
        .or_else(|| husky_path.flatten().filter(|path| path.exists()));

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
    .current_dir(repo.workdir().unwrap_or(repo.git_dir()))
    .stdin(Stdio::piped())
    .spawn()?;

    {
        // Wait for the child process to be ready before writing to stdin.
        // Check if the process has already exited unexpectedly.
        if let Some(status) = child.try_wait()? {
            // Process already exited, don't write to stdin.
            let error = format!("pre-push hook exited early with status: {status}");
            return Ok(HookResult::Failure(ErrorData { error }));
        }

        let remote_commit = repo
            .try_find_reference(&remote_tracking_branch.to_string())?
            .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
            .transpose()?
            .unwrap_or_else(|| repo.object_hash().null());
        // THIS IS WRONG: but is correct in the common case. This also is an issue when the ref is actually pushed,
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

fn path_is_in_husky_dir(repo: &gix::Repository, path: &Path) -> bool {
    let Some(workdir) = repo.workdir() else {
        return false;
    };

    let husky_dir = canonicalize_fallback(workdir.join(".husky"), workdir);
    let path = canonicalize_fallback(path, workdir);
    path.starts_with(husky_dir)
}

fn canonicalize_fallback(path: impl AsRef<Path>, workdir: &Path) -> PathBuf {
    let path = path.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workdir.join(path)
    };
    std::fs::canonicalize(&absolute).unwrap_or(absolute)
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
