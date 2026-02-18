use anyhow::Context as _;
use serde_json::Value;
use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

fn status_json(env: &Sandbox) -> anyhow::Result<Value> {
    let stdout = env
        .but("--json status")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&stdout).context("status output should be valid JSON")
}

fn find_branch<'a>(status: &'a Value, branch_name: &str) -> anyhow::Result<&'a Value> {
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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    anyhow::ensure!(output.status.success(), "failed to read current branch");
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn enter_edit_mode_with_conflicted_commit(env: &Sandbox) -> anyhow::Result<()> {
    env.but("branch new branchB").assert().success();

    env.file("test-file.txt", "line 1\nline 2\nline 3\n");
    env.but("commit -m 'first commit' branchB").assert().success();

    env.file("test-file.txt", "line 1\nline 2\nline 3\nline 4\n");
    env.but("commit -m 'second commit' branchB").assert().success();

    let status_before = status_json(env)?;
    let branch_before = find_branch(&status_before, "branchB")?;
    let first_commit_cli_id = branch_before["commits"]
        .as_array()
        .context("branch commits should be an array")?
        .iter()
        .find(|commit| commit["message"].as_str() == Some("first commit"))
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find first commit cli id")?;

    env.but(format!("rub {first_commit_cli_id} zz")).assert().success();

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

#[test]
fn resolve_status_and_finish_work_in_edit_mode() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.but("resolve status")
        .assert()
        .success()
        .stderr_eq(str![]);

    env.file("test-file.txt", "resolved content\n");
    env.invoke_git("add test-file.txt");

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_cancel_works_in_edit_mode() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.but("resolve cancel --force")
        .assert()
        .success()
        .stderr_eq(str![]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_cancel_requires_force_when_changes_were_made() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.file("test-file.txt", "resolved content with additional edits\n");

    env.but("resolve cancel")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. There are changes that differ from the original commit you were editing. Canceling will drop those changes.

If you want to go through with this, please re-run with `--force`.

If you want to keep the changes you have made, consider finishing the resolution and then moving the changes with the rub command.

"#]]);

    env.but("resolve cancel --force")
        .assert()
        .success()
        .stderr_eq(str![]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}
