use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

/// Get commit SHA from a git reference
fn get_commit_sha(env: &Sandbox, git_ref: &str) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg(git_ref)
        .output()
        .expect("git rev-parse failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Check if a branch contains a commit with the given message substring
fn branch_has_commit_message(env: &Sandbox, branch_name: &str, message_contains: &str) -> bool {
    let result = env.but("status --json").assert().success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout);
    let status: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap())
        .filter(|branch| branch["name"] == branch_name)
        .flat_map(|branch| branch["commits"].as_array().unwrap())
        .any(|commit| {
            commit["message"]
                .as_str()
                .map(|m| m.contains(message_contains))
                .unwrap_or(false)
        })
}

// === Success cases ===

#[test]
fn pick_by_full_sha() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let sha = get_commit_sha(&env, "refs/gitbutler/pickable-first");
    env.but(format!("pick {sha} applied-branch")).assert().success();

    assert!(branch_has_commit_message(
        &env,
        "applied-branch",
        "first pickable commit"
    ));
    Ok(())
}

#[test]
fn pick_by_short_sha() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let short_sha = &get_commit_sha(&env, "refs/gitbutler/pickable-first")[..7];
    env.but(format!("pick {short_sha} applied-branch")).assert().success();

    assert!(branch_has_commit_message(
        &env,
        "applied-branch",
        "first pickable commit"
    ));
    Ok(())
}

#[test]
fn pick_by_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    // When picking by branch name in non-interactive mode, picks the head commit
    env.but("pick unapplied-branch applied-branch").assert().success();

    assert!(branch_has_commit_message(
        &env,
        "applied-branch",
        "second pickable commit"
    ));
    Ok(())
}

#[test]
fn pick_auto_selects_single_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let sha = get_commit_sha(&env, "refs/gitbutler/pickable-first");

    // No target specified - should auto-select the only stack
    let result = env.but(format!("pick {sha}")).assert().success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout);

    assert!(stdout.contains("into branch applied-branch"));
    Ok(())
}

#[test]
fn pick_target_is_case_insensitive() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let sha = get_commit_sha(&env, "refs/gitbutler/pickable-first");
    env.but(format!("pick {sha} APPLIED-BRANCH")).assert().success();

    Ok(())
}

#[test]
fn pick_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    let stack_ids = env.setup_metadata(&["applied-branch"])?;

    let sha = get_commit_sha(&env, "refs/gitbutler/pickable-first");
    let result = env
        .but(format!("--json pick {sha} applied-branch"))
        .allow_json()
        .output()?;

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;

    assert_eq!(json["picked_commit"], sha);
    assert_eq!(json["target_branch"], "applied-branch");
    assert_eq!(json["target_stack_id"], stack_ids[0].to_string());
    Ok(())
}

// === Error cases ===

#[test]
fn pick_invalid_source_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    env.but("pick nonexistent-thing applied-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to pick commit. Source 'nonexistent-thing' is not a valid commit ID, CLI ID, or unapplied branch name.
Run 'but status' to see available CLI IDs, or 'but branch list' to see branches.

"#]]);

    Ok(())
}

#[test]
fn pick_invalid_target_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let sha = get_commit_sha(&env, "refs/gitbutler/pickable-first");
    env.but(format!("pick {sha} nonexistent-branch"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to pick commit. Target branch 'nonexistent-branch' not found among applied stacks.
Available stacks: applied-branch

"#]]);

    Ok(())
}
