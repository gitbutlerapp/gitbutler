//! Put command-specific tests here. They should be focused on what's important for each command.
//! Ideally they *show* the initial state, and the *post* state, to validate the commands actually do what they claim.
//! **Only** test the *happy path* of a typical user journey, while keeping details in unit tests with private module access.
#[cfg(feature = "legacy")]
mod absorb;
mod branch;
#[cfg(feature = "legacy")]
mod claude;
#[cfg(feature = "legacy")]
mod commit;
#[cfg(feature = "legacy")]
mod cursor;
#[cfg(feature = "legacy")]
mod diff;
mod format;
mod gui;
mod help;
mod link;
#[cfg(feature = "legacy")]
mod merge;
#[cfg(feature = "legacy")]
mod r#move;
mod onboarding;
#[cfg(feature = "legacy")]
mod pick;
#[cfg(feature = "legacy")]
mod resolve;
#[cfg(feature = "legacy")]
mod reword;
#[cfg(feature = "legacy")]
mod rub;
#[cfg(feature = "legacy")]
mod setup;
mod skill;
#[cfg(feature = "legacy")]
mod squash;
#[cfg(feature = "legacy")]
mod status;
#[cfg(feature = "legacy")]
mod teardown;

#[cfg(feature = "legacy")]
mod util {
    use anyhow::Context as _;

    use crate::utils::{CommandExt as _, Sandbox};

    /// Create two files `filename1` and `filename2` and commit them to `branch`,
    /// each having two lines, `first_line`, then filler, and a last line that are far enough apart to
    /// ensure that they become 2 hunks when changed.
    pub fn commit_two_files_as_two_hunks_each(
        env: &Sandbox,
        branch: &str,
        filename1: &str,
        filename2: &str,
        first_line: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename1,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.file(
            filename2,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!(
            "commit {branch} -m 'create {filename1} and {filename2}'"
        ))
        .assert()
        .success();
    }

    /// Create a file with `filename`, commit it to `branch`, then edit it once more to have two uncommitted hunks.
    pub fn commit_file_with_worktree_changes_as_two_hunks(
        env: &Sandbox,
        branch: &str,
        filename: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename,
            format!("first\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!("commit {branch} -m {filename}"))
            .assert()
            .success();
        env.file(
            filename,
            format!("firsta\n{}lasta\n", "line\n".repeat(context_distance)),
        );
    }

    /// Return `but status` JSON output as a parsed value.
    pub fn status_json(env: &Sandbox) -> anyhow::Result<serde_json::Value> {
        let output = env.but("--json status").allow_json().output()?;
        serde_json::from_slice(&output.stdout).context("status output should be valid JSON")
    }

    /// Find a branch by name in `status` output.
    pub fn find_branch<'a>(
        status: &'a serde_json::Value,
        branch_name: &str,
    ) -> anyhow::Result<&'a serde_json::Value> {
        status["stacks"]
            .as_array()
            .context("status.stacks should be an array")?
            .iter()
            .flat_map(|stack| {
                stack["branches"]
                    .as_array()
                    .into_iter()
                    .flat_map(|branches| branches.iter())
            })
            .find(|branch| branch["name"].as_str() == Some(branch_name))
            .context("expected branch in status output")
    }

    /// Create a conflicted edit-mode session by reordering commits and entering `resolve`.
    pub fn enter_edit_mode_with_conflicted_commit(env: &Sandbox) -> anyhow::Result<()> {
        env.but("branch new branchB").assert().success();

        env.file("test-file.txt", "line 1\nline 2\nline 3\n");
        env.but("commit -m 'first commit' branchB")
            .assert()
            .success();

        env.file("test-file.txt", "line 1\nline 2\nline 3\nline 4\n");
        env.but("commit -m 'second commit' branchB")
            .assert()
            .success();

        let status_before = status_json(env)?;
        let branch_before = find_branch(&status_before, "branchB")?;
        let first_commit_cli_id = branch_before["commits"]
            .as_array()
            .context("branch commits should be an array")?
            .iter()
            .find(|commit| commit["message"].as_str() == Some("first commit"))
            .and_then(|commit| commit["cliId"].as_str())
            .context("should find first commit cli id")?;

        env.but(format!("rub {first_commit_cli_id} zz"))
            .assert()
            .success();

        let status_after = status_json(env)?;
        let branch_after = find_branch(&status_after, "branchB")?;
        let conflicted_commit_cli_id = branch_after["commits"]
            .as_array()
            .context("branch commits should be an array")?
            .iter()
            .find(|commit| commit["conflicted"].as_bool() == Some(true))
            .and_then(|commit| commit["cliId"].as_str())
            .context("should find conflicted commit cli id")?;

        env.but(format!("resolve {conflicted_commit_cli_id}"))
            .assert()
            .success();
        Ok(())
    }
}
