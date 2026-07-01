use std::path::Path;

use anyhow::{Result, bail};

use crate::WorktreeId;

/// Creates a git worktree.
pub(crate) fn git_worktree_add(
    project_path: &Path,
    path: &Path,
    commit: gix::ObjectId,
) -> Result<()> {
    let output =
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()))
            .current_dir(project_path)
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(path.as_os_str())
            .arg(commit.to_string())
            .stderr(std::process::Stdio::piped())
            .output()?;

    tracing::debug!(
        stdout = %String::from_utf8_lossy(&output.stdout),
        stderr = %String::from_utf8_lossy(&output.stderr),
        "git worktree add"
    );

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "Failed to create worktree\n\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

/// Removes a git worktree
pub(crate) fn git_worktree_remove(project_path: &Path, id: &WorktreeId, force: bool) -> Result<()> {
    let mut command =
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()));
    command.current_dir(project_path);
    command.arg("worktree");
    command.arg("remove");
    command.arg(id.to_os_str());

    if force {
        command.arg("--force");
    }

    let output = command.stderr(std::process::Stdio::piped()).output()?;

    tracing::debug!(
        stdout = %String::from_utf8_lossy(&output.stdout),
        stderr = %String::from_utf8_lossy(&output.stderr),
        "git worktree remove"
    );

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "Failed to remove worktree\n\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    }
}
