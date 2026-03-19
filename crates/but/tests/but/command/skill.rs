use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn skill_check_local_outside_repo_fails() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("skill check --local")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Cannot check local installations: not in a git repository.
Use --global to check global installations, or run from within a repository.

"#]]);
    Ok(())
}

#[test]
fn skill_check_json_output_is_valid() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Check with --global to avoid needing a repo context
    // The JSON output should always be valid even if no skills are found
    let output = env
        .but("skill check --global --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify it's valid JSON
    let json: serde_json::Value = serde_json::from_slice(&output)?;

    // Verify the expected structure
    assert!(json.get("cli_version").is_some(), "should have cli_version");
    assert!(json.get("skills").is_some(), "should have skills array");
    assert!(
        json.get("outdated_count").is_some(),
        "should have outdated_count"
    );

    Ok(())
}

#[test]
fn skill_install_json_outside_repo_requires_path_instead_of_repo_context() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("skill install --json")
        .allow_json()
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: In non-interactive mode, you must specify --path or --detect. Use --path <path> to specify where to install the skill, or --detect to update an existing installation.

"#]]);

    Ok(())
}

#[test]
fn skill_install_path_outside_repo_requires_global() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("skill install --json --path .claude/skills/gitbutler")
        .allow_json()
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Cannot use relative --path outside a git repository unless --global is specified.
Use --global --path <path> for a global installation, use an absolute path, or run from within a repository for local installation.

"#]]);

    Ok(())
}

#[test]
fn skill_install_absolute_path_outside_repo_does_not_require_global() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let install_dir = env.projects_root().join("abs-skill-install");

    let output = env
        .but("")
        .arg("skill")
        .arg("install")
        .arg("--json")
        .arg("--path")
        .arg(&install_dir)
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output)?;
    assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
    let expected_path = install_dir.display().to_string();
    assert_eq!(
        json.get("path").and_then(|v| v.as_str()),
        Some(expected_path.as_str())
    );

    Ok(())
}

#[test]
fn skill_install_surfaces_non_repo_discovery_errors() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let invalid_dir = env.projects_root().join("not-a-directory");
    std::fs::write(&invalid_dir, "not a dir")?;

    let output = env
        .but("")
        .arg("-C")
        .arg(&invalid_dir)
        .arg("skill")
        .arg("install")
        .arg("--json")
        .allow_json()
        .assert()
        .failure();

    let stderr = std::str::from_utf8(&output.get_output().stderr)?;
    assert!(
        stderr.contains("Failed to access a directory, or path is not a directory"),
        "Expected directory access error, got: {stderr}"
    );
    assert!(
        !stderr.contains("In non-interactive mode, you must specify --path"),
        "Unexpected fallback to non-interactive path prompt: {stderr}"
    );

    Ok(())
}
