use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_ctx::Context;
use serde::Serialize;

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

pub fn commit_msg(ctx: &Context, message: String) -> Result<MessageHookResult> {
    let repo = ctx.repo.get()?;
    let Some(hook) = find_commit_hook(&repo, "commit-msg", ctx.legacy_project.husky_hooks_enabled)?
    else {
        return Ok(MessageHookResult::NotConfigured);
    };
    let message_path = repo
        .current_dir()
        .join(repo.git_dir())
        .join("COMMIT_EDITMSG");
    std::fs::write(&message_path, &message).context("failed to prepare commit hook message")?;
    let output = repo.run_hook(
        hook,
        gix::hook::Options {
            arguments: &[message_path.as_os_str()],
            ..Default::default()
        },
    )?;
    let edited = std::fs::read_to_string(&message_path)
        .context("failed to read message after commit hook")?;
    if !output.status.success() {
        return Ok(MessageHookResult::Failure(ErrorData {
            error: join_output(
                output.stdout.to_str_lossy().into_owned(),
                output.stderr.to_str_lossy().into_owned(),
                output.status.code(),
            ),
        }));
    }
    Ok(if edited == message {
        MessageHookResult::Success
    } else {
        MessageHookResult::Message(MessageData { message: edited })
    })
}

pub fn pre_commit_with_tree(ctx: &Context, tree_id: gix::ObjectId) -> Result<HookResult> {
    let repo = ctx.repo.get()?;
    // Back up the index file byte for byte; a round-trip through a tree would fail
    // on an index with unmerged entries (a conflict in an uncommitted file) and
    // could not bring those entries back. A sibling file copy keeps memory flat and
    // lets the restore be a single atomic rename that also keeps the permissions.
    let index_path = repo.current_dir().join(repo.index_path());
    let backup_path = index_path.with_extension("gitbutler-hook-backup");
    let backup_tmp_path = index_path.with_extension("gitbutler-hook-backup.tmp");
    let mut transaction_lock =
        but_core::sync::LockFile::open(index_path.with_extension("gitbutler-hook-lock"))
            .context("failed to open pre-commit index lock")?;
    if !transaction_lock
        .try_lock()
        .context("failed to lock the index for a pre-commit hook")?
    {
        anyhow::bail!("another pre-commit hook is already using the repository index");
    }
    match std::fs::symlink_metadata(&backup_path) {
        Ok(_) => anyhow::bail!(
            "stale pre-commit index backup at '{}'; restore it to '{}' before retrying",
            backup_path.display(),
            index_path.display()
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).context("failed to inspect pre-commit index backup"),
    }
    match std::fs::remove_file(&backup_tmp_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).context("failed to remove stale temporary index backup"),
    }
    let had_index = match std::fs::copy(&index_path, &backup_tmp_path) {
        Ok(_) => {
            // Git compares an index entry's file by content only while the index file isn't
            // newer than the entry, so the restored index keeps its modified time, which a copy
            // doesn't on every platform. Otherwise a same-size edit looks committed.
            let modified = std::fs::metadata(&index_path)?.modified()?;
            std::fs::File::options()
                .write(true)
                .open(&backup_tmp_path)?
                .set_modified(modified)
                .context("failed to keep the index's modified time in its backup")?;
            std::fs::rename(&backup_tmp_path, &backup_path)
                .context("failed to finalize pre-commit index backup")?;
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err).context("failed to back up index for pre-commit hook"),
    };

    // Panic fallback; normal restoration below can report failures to the caller.
    let guard = scopeguard::guard((), |_| {
        if let Err(err) = restore_index(&index_path, &backup_path, had_index) {
            tracing::error!("Failed to reset index: {}", err);
        }
    });

    let hook_result = (|| -> Result<HookResult> {
        {
            let previous = repo.index_or_empty()?;
            repo.index_from_tree_with_index(&tree_id, &previous)?
                .write(Default::default())?;
        }
        let Some(hook) =
            find_commit_hook(&repo, "pre-commit", ctx.legacy_project.husky_hooks_enabled)?
        else {
            return Ok(HookResult::NotConfigured);
        };
        let output = repo.run_hook(
            hook,
            gix::hook::Options {
                environment: &[(OsStr::new("GIT_INDEX_FILE"), index_path.as_os_str())],
                ..Default::default()
            },
        )?;
        // Older GitButler hooks blocked commits on gitbutler/workspace with this marker.
        Ok(
            if output.status.success()
                || output.stdout.contains_str("GITBUTLER_ERROR")
                || output.stderr.contains_str("GITBUTLER_ERROR")
            {
                HookResult::Success
            } else {
                HookResult::Failure(ErrorData {
                    error: join_output(
                        output.stdout.to_str_lossy().into_owned(),
                        output.stderr.to_str_lossy().into_owned(),
                        output.status.code(),
                    ),
                })
            },
        )
    })();

    if let Err(err) = restore_index(&index_path, &backup_path, had_index) {
        drop(guard); // Retry once through the fallback before returning the original error.
        return Err(err);
    }
    scopeguard::ScopeGuard::into_inner(guard);
    hook_result
}

fn restore_index(index_path: &Path, backup_path: &Path, had_index: bool) -> Result<()> {
    if had_index {
        std::fs::rename(backup_path, index_path).context("failed to restore pre-commit index")?;
    } else {
        match std::fs::remove_file(index_path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                return Err(err).context("failed to remove temporary pre-commit index");
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn post_commit(ctx: &Context) -> Result<HookResult> {
    let repo = ctx.repo.get()?;
    let Some(hook) =
        find_commit_hook(&repo, "post-commit", ctx.legacy_project.husky_hooks_enabled)?
    else {
        return Ok(HookResult::NotConfigured);
    };
    let output = repo.run_hook(hook, Default::default())?;
    Ok(if output.status.success() {
        HookResult::Success
    } else {
        HookResult::Failure(ErrorData {
            error: join_output(
                output.stdout.to_str_lossy().into_owned(),
                output.stderr.to_str_lossy().into_owned(),
                output.status.code(),
            ),
        })
    })
}

fn find_commit_hook(
    repo: &gix::Repository,
    name: &str,
    husky_enabled: bool,
) -> Result<Option<PathBuf>> {
    let hook = repo.find_hook(name)?;
    if hook.is_some()
        || !husky_enabled
        || repo
            .config_snapshot()
            .plumbing()
            .string_implicit("core.hooksPath")
            .is_some()
    {
        return Ok(hook);
    }
    // An inactive default hook still takes precedence over the optional Husky fallback.
    if repo.hooks_directory()?.join(name).try_exists()? {
        return Ok(None);
    }
    let Some(workdir) = repo.workdir() else {
        return Ok(None);
    };
    let path = repo.current_dir().join(workdir).join(".husky").join(name);
    Ok(gix::sec::identity::is_path_executable_by_current_user(&path)?.then_some(path))
}

/// Run pre-push with refspec information inferred from a remote-tracking branch.
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
    let mut hook_path = repo
        .find_hook("pre-push")?
        .filter(|path| run_husky_hooks || !path_is_in_husky_dir(repo, path));
    if hook_path.is_none()
        && run_husky_hooks
        && !repo.hooks_directory()?.join("pre-push").try_exists()?
        && let Some(workdir) = repo.workdir()
    {
        let path = repo.current_dir().join(workdir).join(".husky/pre-push");
        hook_path = gix::sec::identity::is_path_executable_by_current_user(&path)?.then_some(path);
    }

    let Some(hook_path) = hook_path else {
        return Ok(HookResult::NotConfigured);
    };

    let remote_commit = repo
        .try_find_reference(&remote_tracking_branch.to_string())?
        .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
        .transpose()?
        .unwrap_or_else(|| repo.object_hash().null());
    // THIS IS WRONG: but is correct in the common case. This also is an issue when the ref is actually pushed,
    // but we can fix it when moving everything to `gix`.
    let local_tracking_branch_deduced = format!("refs/heads/{}", remote_tracking_branch.branch());
    let refspec = format!(
        "{local_tracking_branch_deduced} {local_commit} {remote_tracking_branch} {remote_commit}\n"
    );
    let output = repo.run_hook(
        hook_path,
        gix::hook::Options {
            arguments: &[OsStr::new(remote_name), OsStr::new(remote_url)],
            stdin: Some(refspec.as_bytes()),
            ..Default::default()
        },
    )?;
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

    let workdir = repo.current_dir().join(workdir);
    let husky_dir = canonicalize_fallback(workdir.join(".husky"), &workdir);
    let path = canonicalize_fallback(path, &workdir);
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
    if stdout.is_empty() && stderr.is_empty() {
        return format!("hook produced no output{code}");
    } else if stdout.is_empty() {
        return stderr;
    } else if stderr.is_empty() {
        return stdout;
    }
    format!("stdout:\n{stdout}\n\nstderr:\n{stderr}{code}")
}
