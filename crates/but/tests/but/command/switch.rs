use snapbox::str;

#[cfg(feature = "legacy")]
use crate::utils::CommandExt as _;

#[test]
fn switches_to_existing_branch_by_short_name() {
    let env = switch_env();

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
}

#[test]
fn switches_to_existing_branch_by_full_ref() {
    let env = switch_env();

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
}

#[test]
fn switches_to_existing_branch_with_remote_like_name() {
    let env = switch_env();
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
}

#[cfg(feature = "legacy")]
#[test]
fn switches_to_existing_branch_by_workspace_cli_id() {
    let env = switch_env();

    assert_workspace_status(&env);

    let status = status_json(&env);
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
}

#[cfg(feature = "legacy")]
#[test]
fn switching_to_lower_branch_only_shows_it_in_single_branch_mode() {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-two-dependent-branches",
    );
    env.setup_single_stack_metadata_at_target(&["B", "A"], "origin/main");

    // The managed workspace shows the complete stack before switching.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("switch A").assert().success();

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
    // Switching to A should hide B above it from single-branch status.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn switching_to_reordered_empty_branch_preserves_lower_branches() {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.but("branch new A").assert().success();
    env.but("branch new B").assert().success();
    env.but("branch new D --above A").assert().success();
    env.but("branch new C --above D").assert().success();
    env.but("move A --above D").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B] (no commits)
├╯
┊
┊╭┄ h0 [C] (no commits)
┊│
┊├┄ i0 [A] (no commits)
┊│
┊├┄ j0 [D] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("switch A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);
    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
┊│
┊├┄ h0 [D] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switches_back_to_workspace() {
    let env = switch_env();
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
}

#[test]
fn creates_named_branch_and_switches_to_it() {
    let env = switch_env();

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch --new my-feature")
        .assert()
        .success()
        .stderr_eq(str![[r#"
⚠ `--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead

"#]])
        .stdout_eq(str![[r#"
Created branch 'my-feature'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "my-feature");
    assert_eq!(
        env.invoke_git("rev-parse my-feature"),
        env.invoke_git("rev-parse main")
    );
}

#[test]
fn creates_generated_branch_and_switches_to_it() {
    let env = switch_env();

    #[cfg(feature = "legacy")]
    assert_workspace_status(&env);

    env.but("switch --new")
        .assert()
        .success()
        .stderr_eq(str![[r#"
⚠ `--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead

"#]])
        .stdout_eq(str![[r#"
Created branch 'a-branch-1'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "a-branch-1");
    assert_eq!(
        env.invoke_git("rev-parse a-branch-1"),
        env.invoke_git("rev-parse main")
    );
}

#[test]
fn rejects_workspace_with_target() {
    let env = switch_env();

    env.but("switch --workspace A")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: the argument '--workspace' cannot be used with '[TARGET]'

Usage: but switch <TARGET|--workspace|--new>

For more information, try '--help'.

"#]]);
}

#[test]
fn rejects_remote_branch() {
    let env = switch_env();

    env.but("switch origin/main")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Can only switch to local branches, got 'origin/main'

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn rejects_non_branch_cli_id() {
    let env = switch_env();
    let status = status_json(&env);
    let commit_cli_id = status["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .expect("commit cli id should exist");

    env.but(format!("switch {commit_cli_id}"))
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Could not find branch: 'tpm'

Hint: Run `but status` for applicable targets.

"#]]);
}

fn switch_env() -> crate::utils::Sandbox {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env
}

#[cfg(feature = "legacy")]
fn status_json(env: &crate::utils::Sandbox) -> serde_json::Value {
    let output = env.but("--json status").allow_json().output().unwrap();
    serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("status output should be valid JSON: {err}"))
        .unwrap()
}

#[cfg(feature = "legacy")]
fn assert_workspace_status(env: &crate::utils::Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
