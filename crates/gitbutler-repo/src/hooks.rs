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

pub fn pre_push(ctx: &CommandContext, remote_name: &str, remote_url: &str) -> Result<HookResult> {
    // Since git2-hooks doesn't support pre-push yet, we implement it ourselves
    // following the same pattern as the existing hooks
    let repo = ctx.repo();
    let hooks_path = repo.path().join("hooks").join("pre-push");
    let husky_path = repo
        .path()
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join(".husky").join("pre-push"));

    // Check for hook in .git/hooks/pre-push first, then ../.husky/pre-push
    let hook_path = if hooks_path.exists() {
        Some(hooks_path)
    } else if let Some(husky) = husky_path {
        if husky.exists() {
            Some(husky)
        } else {
            None
        }
    } else {
        None
    };

    let Some(hook_path) = hook_path else {
        return Ok(HookResult::NotConfigured);
    };

    // Execute the pre-push hook with remote name and URL as arguments
    let output = std::process::Command::new(&hook_path)
        .arg(remote_name)
        .arg(remote_url)
        .current_dir(repo.workdir().unwrap_or_else(|| repo.path()))
        .output()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use gitbutler_testsupport::TestProject;
    use std::fs;

    #[test]
    fn test_pre_push_hook_not_configured() {
        let test_project = TestProject::default();
        let project = test_project.project();
        let ctx = &CommandContext::open(project, but_settings::AppSettings::default()).unwrap();

        let result = crate::hooks::pre_push(ctx, "origin", "https://github.com/test/repo.git");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), crate::hooks::HookResult::NotConfigured);
    }

    #[test]
    fn test_pre_push_hook_success() {
        let test_project = TestProject::default();
        let project = test_project.project();
        let ctx = &CommandContext::open(project, but_settings::AppSettings::default()).unwrap();

        // Create a simple pre-push hook that succeeds
        let hooks_dir = ctx.repo().path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("pre-push");

        #[cfg(unix)]
        fs::write(&hook_path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(windows)]
        fs::write(&hook_path, "@echo off\nexit 0\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = crate::hooks::pre_push(ctx, "origin", "https://github.com/test/repo.git");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), crate::hooks::HookResult::Success);
    }

    #[test]
    fn test_pre_push_hook_failure() {
        let test_project = TestProject::default();
        let project = test_project.project();
        let ctx = &CommandContext::open(project, but_settings::AppSettings::default()).unwrap();

        // Create a simple pre-push hook that fails
        let hooks_dir = ctx.repo().path().join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_path = hooks_dir.join("pre-push");

        #[cfg(unix)]
        fs::write(&hook_path, "#!/bin/sh\necho 'Hook failed'\nexit 1\n").unwrap();
        #[cfg(windows)]
        fs::write(&hook_path, "@echo off\necho Hook failed\nexit 1\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = crate::hooks::pre_push(ctx, "origin", "https://github.com/test/repo.git");
        assert!(result.is_ok());
        match result.unwrap() {
            crate::hooks::HookResult::Failure(error_data) => {
                assert!(error_data.error.contains("Hook failed"));
            }
            _ => panic!("Expected hook failure"),
        }
    }
}
