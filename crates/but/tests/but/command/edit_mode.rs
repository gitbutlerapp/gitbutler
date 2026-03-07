use anyhow::Context as _;
use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

fn status_json(env: &Sandbox) -> anyhow::Result<serde_json::Value> {
    let output = env.but("--json status").allow_json().output()?;
    serde_json::from_slice(&output.stdout).context("status output should be valid JSON")
}

fn find_branch<'a>(
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

fn current_branch_name(env: &Sandbox) -> anyhow::Result<String> {
    let repo = env.open_repo()?;
    let _ = repo
        .rev_parse_single("HEAD")
        .context("HEAD should resolve")?;
    repo.head_name()?
        .map(|name| name.as_ref().shorten().to_string())
        .context("HEAD should point to a branch")
}

/// Enter edit mode for a non-conflicted commit on branchB.
/// Creates branchB with two commits, then enters edit mode on the first one.
fn enter_edit_mode_for_commit(env: &Sandbox) -> anyhow::Result<String> {
    env.but("branch new branchB").assert().success();

    env.file("test-file.txt", "line 1\nline 2\nline 3\n");
    env.but("commit -m 'first commit' branchB")
        .assert()
        .success();

    env.file("test-file.txt", "line 1\nline 2\nline 3\nline 4\n");
    env.but("commit -m 'second commit' branchB")
        .assert()
        .success();

    let status = status_json(env)?;
    let branch = find_branch(&status, "branchB")?;
    let first_commit_cli_id = branch["commits"]
        .as_array()
        .context("branch commits should be an array")?
        .iter()
        .find(|commit| commit["message"].as_str() == Some("first commit"))
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find first commit cli id")?
        .to_string();

    env.but(format!("edit-mode {first_commit_cli_id}"))
        .assert()
        .success();

    Ok(first_commit_cli_id)
}

#[test]
fn edit_mode_enter_and_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_for_commit(&env)?;

    assert_eq!(current_branch_name(&env)?, "gitbutler/edit");

    env.but("edit-mode status")
        .assert()
        .success()
        .stderr_eq(str![""]);

    // `but status` should also succeed in edit mode (non-conflicted commit shows edit-mode status)
    env.but("status").assert().success().stderr_eq(str![""]);

    Ok(())
}

#[test]
fn edit_mode_finish() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_for_commit(&env)?;

    // Make a change
    env.file("test-file.txt", "line 1\nline 2 modified\nline 3\n");

    env.but("edit-mode finish")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn edit_mode_cancel_no_changes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_for_commit(&env)?;

    env.but("edit-mode cancel")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn edit_mode_cancel_requires_force_when_changes_were_made() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_for_commit(&env)?;

    env.file("test-file.txt", "modified content\n");

    env.but("edit-mode cancel")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle edit mode. There are changes that differ from the original commit you were editing. Canceling will drop those changes.

If you want to go through with this, please re-run with `--force`.

If you want to keep the changes you have made, consider finishing the edit with `but edit-mode finish`.

"#]]);

    env.but("edit-mode cancel --force")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}
