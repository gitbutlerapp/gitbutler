use std::path::Path;

use anyhow::{Result, bail};
use tracing::{Level, event};

pub(crate) fn git_worktree_add(
    project_path: &Path,
    path: &Path,
    branch_name: &str,
    commit: gix::ObjectId,
) -> Result<()> {
    let output =
        std::process::Command::from(gix::command::prepare(gix::path::env::exe_invocation()))
            .current_dir(project_path)
            .arg("worktree")
            .arg("add")
            .args(["-B", branch_name])
            .arg(path.as_os_str())
            .arg(commit.to_string())
            .output()?;

    event!(Level::INFO, "{}", str::from_utf8(&output.stdout)?);
    event!(Level::ERROR, "{}", str::from_utf8(&output.stderr)?);

    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "Failed to create worktree\n\n{}",
            str::from_utf8(&output.stderr).unwrap_or("")
        )
    }
}
