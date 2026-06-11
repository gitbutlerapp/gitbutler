use std::path::Path;

use anyhow::{Result, bail};
use bstr::BString;

use crate::{WORKTREE_BRANCH_NAMESPACE, WorktreeId};

/// Creates a git worktree.
///
/// Git does not accept fully qualified branch names. The given partial ref will
/// be written out under `refs/heads`
///
/// Returns the full reference.
pub(crate) fn git_worktree_add(
    project_path: &Path,
    path: &Path,
    branch_name: &gix::refs::PartialNameRef,
    commit: gix::ObjectId,
) -> Result<gix::refs::FullName> {
    let output =
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()))
            .current_dir(project_path)
            .arg("worktree")
            .arg("add")
            .args(["-B", &branch_name.to_string()])
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
        let mut out = BString::from(b"refs/heads/");
        out.extend_from_slice(branch_name.as_bstr());
        Ok(gix::refs::FullName::try_from(out)?)
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

/// Deletes the `refs/heads/gitbutler/worktree/<id>` branch that was created
/// alongside the worktree, if it still exists.
///
/// Branches outside that namespace are never touched, so anything the user
/// checked out in the worktree themselves survives.
pub(crate) fn delete_worktree_branch(repo: &gix::Repository, id: &WorktreeId) -> Result<()> {
    let branch_name = format!("refs/heads/{WORKTREE_BRANCH_NAMESPACE}{}", id.as_bstr());
    if let Some(reference) = repo.try_find_reference(&*branch_name)? {
        reference.delete()?;
    }
    Ok(())
}
