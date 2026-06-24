use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

fn assert_default_policy(policy: &str) {
    assert!(
        policy.contains("<!-- gitbutler-agent-setup:start -->"),
        "policy should include the managed block start marker, got: {policy}"
    );
    assert!(
        policy
            .contains("Use GitButler (`but`) for version-control inspection and write operations"),
        "policy should include baseline GitButler write guidance, got: {policy}"
    );
    assert!(
        policy.contains("otherwise modify another agent's work"),
        "policy should include multi-agent isolation guidance, got: {policy}"
    );
    assert!(
        policy.contains("amend an unpublished local commit"),
        "policy should include default fold-fixes preference, got: {policy}"
    );
    assert!(
        policy.contains("Use GitButler to move the relevant changes"),
        "policy should include default amend guidance, got: {policy}"
    );
    assert!(
        policy.contains("If one file contains unrelated changes"),
        "policy should include default split suggestion preference, got: {policy}"
    );
    assert!(
        policy.contains("<!-- gitbutler-agent-setup:end -->"),
        "policy should include the managed block end marker, got: {policy}"
    );
}

#[test]
fn agent_setup_print_outputs_default_managed_policy() -> anyhow::Result<()> {
    let env = Sandbox::empty();

    let output = env
        .but("agent setup --print")
        .assert()
        .success()
        .stderr_eq(str![[]])
        .get_output()
        .stdout
        .clone();
    let stdout = std::str::from_utf8(&output)?;

    assert_default_policy(stdout);

    Ok(())
}

#[test]
fn agent_setup_print_json_outputs_policy_field() -> anyhow::Result<()> {
    let env = Sandbox::empty();

    let output = env
        .but("--format json agent setup --print")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![[]])
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output)?;
    let policy = json
        .get("policy")
        .and_then(|value| value.as_str())
        .expect("JSON output should include a string policy field");

    assert_default_policy(policy);

    Ok(())
}

#[test]
fn agent_setup_without_tty_points_to_print_mode() {
    let env = Sandbox::empty();

    env.but("agent setup")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Interactive setup requires a terminal. Use `but agent setup --print` to print the default instructions without modifying files.

"#]]);
}
