use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use git2_hooks::{self, HookResult as H, HookRunResponse};
use serde::Serialize;

use crate::managed_hooks::get_hooks_dir;

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
    #[expect(deprecated, reason = "libgit2 hook adapter boundary")]
    match git2_hooks::hooks_commit_msg(
        &*ctx.git2_repo.get()?,
        husky_search_paths(ctx),
        &mut message,
    )? {
        H::NoHookFound => Ok(MessageHookResult::NotConfigured),
        H::Run(HookRunResponse {
            stdout,
            stderr,
            code,
            ..
        }) => {
            if code == 0 {
                match message == original_message {
                    true => Ok(MessageHookResult::Success),
                    false => Ok(MessageHookResult::Message(MessageData { message })),
                }
            } else {
                let error = join_output(stdout, stderr, Some(code));
                Ok(MessageHookResult::Failure(ErrorData { error }))
            }
        }
    }
}

pub fn pre_commit_with_tree(ctx: &Context, tree_id: gix::ObjectId) -> Result<HookResult> {
    #[expect(deprecated, reason = "libgit2 hook/index adapter boundary")]
    let repo = &*ctx.git2_repo.get()?;
    // Back up the index file byte for byte; a round-trip through a tree would fail
    // on an index with unmerged entries (a conflict in an uncommitted file) and
    // could not bring those entries back. A sibling file copy keeps memory flat and
    // lets the restore be a single atomic rename that also keeps the permissions.
    let index_path = repo
        .index()?
        .path()
        .context("repository index has no backing file")?
        .to_owned();
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
            std::fs::rename(&backup_tmp_path, &backup_path)
                .context("failed to finalize pre-commit index backup")?;
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err).context("failed to back up index for pre-commit hook"),
    };

    // Panic fallback; normal restoration below can report failures to the caller.
    let guard = scopeguard::guard((), |_| {
        if let Err(err) = restore_index(repo, &index_path, &backup_path, had_index) {
            tracing::error!("Failed to reset index: {}", err);
        }
    });

    let hook_result = (|| -> Result<HookResult> {
        {
            let mut index = repo.index()?;
            index.read_tree(&repo.find_tree(tree_id.to_git2())?)?;
            index.write()?;
        }

        Ok(
            match git2_hooks::hooks_pre_commit(repo, husky_search_paths(ctx))? {
                H::NoHookFound => HookResult::NotConfigured,
                H::Run(HookRunResponse {
                    stdout,
                    stderr,
                    code,
                    ..
                }) => {
                    if code == 0 {
                        HookResult::Success
                    } else {
                        // If the output contains GITBUTLER_ERROR, it's our managed hook blocking
                        // commits on gitbutler/workspace - this is expected behavior, not a failure
                        if stdout.contains("GITBUTLER_ERROR") || stderr.contains("GITBUTLER_ERROR")
                        {
                            HookResult::Success
                        } else {
                            let error = join_output(stdout, stderr, Some(code));
                            HookResult::Failure(ErrorData { error })
                        }
                    }
                }
            },
        )
    })();

    if let Err(err) = restore_index(repo, &index_path, &backup_path, had_index) {
        drop(guard); // Retry once through the fallback before returning the original error.
        return Err(err);
    }
    scopeguard::ScopeGuard::into_inner(guard);
    hook_result
}

fn restore_index(
    repo: &git2::Repository,
    index_path: &Path,
    backup_path: &Path,
    had_index: bool,
) -> Result<()> {
    if had_index {
        std::fs::rename(backup_path, index_path).context("failed to restore pre-commit index")?;
        // Refresh the in-memory index from the restored file.
        repo.index()?
            .read(true)
            .context("failed to reload restored pre-commit index")?;
    } else {
        match std::fs::remove_file(index_path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                return Err(err).context("failed to remove temporary pre-commit index");
            }
            _ => {}
        }
        repo.index()?.clear()?;
    }
    Ok(())
}

pub fn post_commit(ctx: &Context) -> Result<HookResult> {
    #[expect(deprecated, reason = "libgit2 hook adapter boundary")]
    match git2_hooks::hooks_post_commit(&*ctx.git2_repo.get()?, husky_search_paths(ctx))? {
        H::NoHookFound => Ok(HookResult::NotConfigured),
        H::Run(HookRunResponse {
            stdout,
            stderr,
            code,
            ..
        }) => {
            if code == 0 {
                Ok(HookResult::Success)
            } else {
                let error = join_output(stdout, stderr, Some(code));
                Ok(HookResult::Failure(ErrorData { error }))
            }
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
    let hooks_dir = get_hooks_dir(repo);
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
        let refspec = format!(
            "{local_tracking_branch_deduced} {local_commit} {remote_tracking_branch} {remote_commit}\n"
        );
        // Hooks may exit before reading stdin if they don't need the refspec info.
        // The actual success/failure is determined by the exit code via wait_with_output() below.
        if let Err(err) = stdin.write_all(refspec.as_bytes())
            && err.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(err.into());
        }
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
    if stdout.is_empty() && stderr.is_empty() {
        return format!("hook produced no output{code}");
    } else if stdout.is_empty() {
        return stderr;
    } else if stderr.is_empty() {
        return stdout;
    }
    format!("stdout:\n{stdout}\n\nstderr:\n{stderr}{code}")
}

#[cfg(test)]
mod tests {
    use super::join_output;

    /// A hook that explains its refusal on stderr says nothing otherwise, so the explanation is
    /// the whole of what the user has to go on.
    #[test]
    fn stderr_only_output_is_reported() {
        assert_eq!(
            join_output(String::new(), "gate says no\n".into(), Some(1)),
            "gate says no\n"
        );
    }

    #[test]
    fn stdout_only_output_is_reported() {
        assert_eq!(
            join_output("formatted 3 files\n".into(), String::new(), Some(1)),
            "formatted 3 files\n"
        );
    }

    #[test]
    fn both_streams_are_labelled() {
        assert_eq!(
            join_output("out".into(), "err".into(), Some(2)),
            "stdout:\nout\n\nstderr:\nerr (Exit Code 2)"
        );
    }

    #[test]
    fn a_silent_hook_says_so_with_its_exit_code() {
        assert_eq!(
            join_output(String::new(), String::new(), Some(1)),
            "hook produced no output (Exit Code 1)"
        );
    }
}
