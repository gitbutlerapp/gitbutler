use std::path::Path;

use anyhow::{Result, bail};
use bstr::BString;

use crate::WorktreeId;

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
            .output()?;

    tracing::info!("{}", str::from_utf8(&output.stdout)?);
    tracing::error!("{}", str::from_utf8(&output.stderr)?);

    if output.status.success() {
        let mut out = BString::from(b"refs/heads/");
        out.extend_from_slice(branch_name.as_bstr());
        Ok(gix::refs::FullName::try_from(out)?)
    } else {
        bail!(
            "Failed to create worktree\n\n{}",
            str::from_utf8(&output.stderr).unwrap_or("")
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
    command.arg(id.as_str());

    if force {
        command.arg("--force");
    }

    let output = command.output()?;

    tracing::info!("{}", str::from_utf8(&output.stdout)?);
    tracing::error!("{}", str::from_utf8(&output.stderr)?);

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "Failed to create worktree\n\n{}",
            str::from_utf8(&output.stderr).unwrap_or("")
        )
    }
}
