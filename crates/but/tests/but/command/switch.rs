use snapbox::str;

#[cfg(feature = "legacy")]
use crate::utils::CommandExt as _;

#[test]
fn switches_to_existing_branch_by_short_name() -> anyhow::Result<()> {
    let env = switch_env()?;

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
    Ok(())
}

#[test]
fn switches_to_existing_branch_by_full_ref() -> anyhow::Result<()> {
    let env = switch_env()?;

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch refs/heads/A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
    Ok(())
}

#[test]
fn switches_to_existing_branch_with_remote_like_name() -> anyhow::Result<()> {
    let env = switch_env()?;
    env.invoke_git("branch origin/main main");

    env.but("switch origin/main")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'origin/main'

"#]]);

    assert_eq!(
        env.invoke_git("rev-parse --symbolic-full-name HEAD"),
        "refs/heads/origin/main"
    );
    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn switches_to_existing_branch_by_workspace_cli_id() -> anyhow::Result<()> {
    let env = switch_env()?;

    assert_workspace_status(&env);

    let status = status_json(&env)?;
    let branch_cli_id = status["stacks"][0]["branches"][0]["cliId"]
        .as_str()
        .expect("branch cli id should exist");

    env.but(format!("switch {branch_cli_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
    Ok(())
}

#[test]
fn switches_back_to_workspace() -> anyhow::Result<()> {
    let env = switch_env()?;
    env.invoke_git("checkout A");

    env.but("switch --workspace")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref HEAD"),
        "gitbutler/workspace"
    );

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);
    Ok(())
}

#[test]
fn creates_named_branch_and_switches_to_it() -> anyhow::Result<()> {
    let env = switch_env()?;

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch --new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created and switched to branch 'my-feature'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "my-feature");
    assert_eq!(
        env.invoke_git("rev-parse my-feature"),
        env.invoke_git("rev-parse main")
    );
    Ok(())
}

#[test]
fn creates_generated_branch_and_switches_to_it() -> anyhow::Result<()> {
    let env = switch_env()?;

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch --new")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created and switched to branch 'a-branch-1'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "a-branch-1");
    assert_eq!(
        env.invoke_git("rev-parse a-branch-1"),
        env.invoke_git("rev-parse main")
    );
    Ok(())
}

#[test]
fn rejects_workspace_with_target() -> anyhow::Result<()> {
    let env = switch_env()?;

    env.but("switch --workspace A")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: the argument '--workspace' cannot be used with '[TARGET]'

Usage: but switch <TARGET|--workspace|--new>

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn rejects_remote_branch() -> anyhow::Result<()> {
    let env = switch_env()?;

    env.but("switch origin/main")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Can only switch to local branches, got 'origin/main'

"#]]);

    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn rejects_non_branch_cli_id() -> anyhow::Result<()> {
    let env = switch_env()?;
    let status = status_json(&env)?;
    let commit_cli_id = status["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .expect("commit cli id should exist");

    env.but(format!("switch {commit_cli_id}"))
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(format!(
            "Error: Invalid branch. '{commit_cli_id}' is a commit\n"
        ));

    Ok(())
}

fn switch_env() -> anyhow::Result<crate::utils::Sandbox> {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    Ok(env)
}

#[cfg(feature = "legacy")]
fn status_json(env: &crate::utils::Sandbox) -> anyhow::Result<serde_json::Value> {
    let output = env.but("--format json status").allow_json().output()?;
    serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("status output should be valid JSON: {err}"))
}

#[cfg(feature = "legacy")]
fn assert_workspace_status(env: &crate::utils::Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
