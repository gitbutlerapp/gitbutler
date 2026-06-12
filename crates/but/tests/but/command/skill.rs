use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

fn relative_agent_skill_path(agent_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(agent_dir)
        .join("skills")
        .join("gitbutler")
}

fn path_ends_with_gitbutler_agents_dir(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.ends_with("/.agents/skills/gitbutler")
        || normalized.ends_with(".agents/skills/gitbutler")
}

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
        .but("skill check --global --format json")
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

    env.but("skill install --format json")
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
    let install_path = relative_agent_skill_path(".claude");

    env.but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "json"])
        .arg("--path")
        .arg(&install_path)
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
        .args(["--format", "json"])
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
fn skill_install_agent_outputs_success_message() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let install_dir = env.projects_root().join("agent-skill-install");

    let output = env
        .but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "agent", "--global"])
        .arg("--path")
        .arg(&install_dir)
        .assert()
        .success()
        .stderr_eq(str![[]])
        .get_output()
        .stdout
        .clone();

    let stdout = std::str::from_utf8(&output)?;
    assert!(
        stdout.contains("GitButler skill installed successfully"),
        "agent skill install should print the human success message, got: {stdout}"
    );
    assert!(
        stdout.contains("Files installed:"),
        "agent skill install should print installed files, got: {stdout}"
    );

    Ok(())
}

#[test]
fn skill_check_detects_agent_skills_installation_in_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;
    let install_path = relative_agent_skill_path(".agents");

    env.but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "json"])
        .arg("--path")
        .arg(&install_path)
        .allow_json()
        .assert()
        .success();

    let output = env
        .but("skill check --local --format json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output)?;
    let skills = json
        .get("skills")
        .and_then(|value| value.as_array())
        .expect("skills array should be present");

    assert!(
        skills.iter().any(|skill| {
            skill
                .get("path")
                .and_then(|value| value.as_str())
                .is_some_and(path_ends_with_gitbutler_agents_dir)
                && skill.get("format_name").and_then(|value| value.as_str()) == Some("Agent Skills")
                && skill.get("scope").and_then(|value| value.as_str()) == Some("local")
        }),
        "expected Agent Skills installation in .agents/skills/gitbutler, got: {skills:?}"
    );

    Ok(())
}

#[test]
fn skill_install_detect_finds_agent_skills_installation_in_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;
    let install_path = relative_agent_skill_path(".agents");

    env.but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "json"])
        .arg("--path")
        .arg(&install_path)
        .allow_json()
        .assert()
        .success();

    let output = env
        .but("skill install --format json --detect")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output)?;
    assert!(
        json.get("path")
            .and_then(|value| value.as_str())
            .is_some_and(path_ends_with_gitbutler_agents_dir),
        "expected detect to reuse .agents/skills/gitbutler, got: {json:?}"
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
        .args(["--format", "json"])
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
