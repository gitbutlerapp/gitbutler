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
fn skill_check_local_outside_repo_fails() {
    let env = Sandbox::empty();

    env.but("skill check --local")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Cannot check local installations: not in a git repository.
Use --global to check global installations, or run from within a repository.

"#]]);
}

#[test]
fn skill_check_json_output_is_valid() -> anyhow::Result<()> {
    let env = Sandbox::empty();

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
fn skill_install_json_outside_repo_requires_path_instead_of_repo_context() {
    let env = Sandbox::empty();

    env.but("skill install --format json")
        .allow_json()
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: No supported agent was detected. In non-interactive mode, specify --path or --detect. Use --path <path> to choose an installation directory, or --detect to update an existing installation.

"#]]);
}

#[test]
fn skill_install_bare_defaults_to_detected_agents_global_dir() {
    // A detected agent running the bare command without a terminal gets its
    // own global skill directory instead of the interactive wizard - this is
    // the command the not-installed status notice suggests.
    for (agent, agent_dir) in [("codex", ".codex"), ("kiro", ".kiro"), ("junie", ".junie")] {
        let env = Sandbox::empty();
        let output = env
            .but("skill install")
            .env("AI_AGENT", agent)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let skill_md = env
            .home_dir()
            .join(relative_agent_skill_path(agent_dir))
            .join("SKILL.md");
        assert!(
            skill_md.is_file(),
            "the {agent} skill lands in its global directory under the sandboxed home"
        );

        // A skill installed mid-session is invisible to the agent's harness until
        // the next session, so the agent is pointed at the file directly.
        let stdout = String::from_utf8_lossy(&output);
        assert!(
            !stdout.contains("AGENT ACTION REQUIRED"),
            "{agent} installation should finish without another action request"
        );
        assert!(
            stdout.contains("To use it in this session, read "),
            "an agent caller is told to read the skill now, got: {stdout}"
        );
    }
}

#[test]
fn agent_skill_notice_gating() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    // Only human-text output can carry the notice; JSON output skips the check.
    let json_run = env
        .but("--format json alias list")
        .env("AI_AGENT", "codex")
        .allow_json()
        .output()
        .expect("status --format json runs");
    assert!(json_run.status.success());
    assert!(
        !String::from_utf8_lossy(&json_run.stdout).contains("AGENT ACTION REQUIRED"),
        "JSON status output carries no skill notice"
    );

    // A human-text agent run delivers the notice, leading the output so
    // output-trimming pipes like `head` keep it.
    let stdout_of = || {
        let out = env
            .but("alias list")
            .env("AI_AGENT", "codex")
            .output()
            .expect("alias list runs");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    // Wording is pinned by the status snapshot; this asserts placement only.
    let stdout = stdout_of();
    assert!(
        stdout.starts_with("⚠ AGENT ACTION REQUIRED"),
        "the notice must lead the output, got: {stdout}"
    );

    // The not-installed notice is not debounced - every session and every
    // agent is nudged on each status until the skill is installed, rather
    // than one delivery silencing all others for hours.
    assert!(
        stdout_of().contains("AGENT ACTION REQUIRED"),
        "the not-installed notice keeps showing until the skill is installed"
    );

    let failed = env
        .but("alias add 'bad name' status")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias add runs");
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stdout).starts_with("⚠ AGENT ACTION REQUIRED"),
        "failed normal commands receive the same pre-dispatch notice"
    );
}

#[test]
fn agent_skill_notice_is_scoped_to_the_driving_agents_format() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    // A Cursor session installs the skill into its own format directory.
    env.but("skill install")
        .env("AI_AGENT", "cursor")
        .assert()
        .success();
    assert!(
        env.home_dir()
            .join(".cursor")
            .join("skills")
            .join("gitbutler")
            .join("SKILL.md")
            .is_file(),
        "the Cursor install landed in the sandboxed home"
    );

    // Claude Code cannot read `.cursor/skills`, so it must still be told to
    // install its own copy - the Cursor install does not count for it.
    let claude = env
        .but("alias list")
        .env("AI_AGENT", "claude-code")
        .output()
        .expect("alias list runs");
    assert!(claude.status.success());
    let stdout = String::from_utf8_lossy(&claude.stdout);
    assert!(
        stdout.contains("AGENT ACTION REQUIRED")
            && stdout.contains("Install the GitButler skill before continuing"),
        "another agent's install must not silence Claude Code, got: {stdout}"
    );

    // The Cursor session, whose format holds the install, is satisfied.
    let cursor = env
        .but("alias list")
        .env("AI_AGENT", "cursor")
        .output()
        .expect("alias list runs");
    assert!(cursor.status.success());
    assert!(
        !String::from_utf8_lossy(&cursor.stdout).contains("AGENT ACTION REQUIRED"),
        "the agent whose format holds the install gets no notice"
    );

    env.but("skill install --path .claude/skills/gitbutler")
        .assert()
        .success();
    let claude = env
        .but("alias list")
        .env("AI_AGENT", "claude-code")
        .output()
        .expect("alias list runs");
    assert!(
        !String::from_utf8_lossy(&claude.stdout).contains("AGENT ACTION REQUIRED"),
        "a repository-local install also satisfies its agent"
    );
}

#[test]
fn agent_skill_notice_accepts_compatible_shared_local_install() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.but("skill install --path .agents/skills/gitbutler")
        .assert()
        .success();

    for agent in ["opencode", "devin"] {
        let output = env
            .but("alias list")
            .env("AI_AGENT", agent)
            .output()
            .expect("alias list runs");
        assert!(output.status.success());
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains("AGENT ACTION REQUIRED"),
            "{agent} should accept the shared local skill format"
        );
    }
}

#[test]
fn agent_skill_notice_reports_a_stale_local_skill_despite_a_current_global_copy() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("skill install --path .codex/skills/gitbutler")
        .assert()
        .success();
    std::fs::write(
        env.projects_root().join(".codex/skills/gitbutler/SKILL.md"),
        "---\nname: but\nversion: old\n---\n",
    )
    .unwrap();
    env.but("skill install")
        .env("AI_AGENT", "codex")
        .assert()
        .success();

    let output = env
        .but("alias list")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias list runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.starts_with("⚠ AGENT ACTION REQUIRED")
            && stdout.contains("but skill check --update"),
        "the stale local copy must not hide behind the current global copy, got: {stdout}"
    );
}

#[test]
fn skill_check_update_repairs_a_stale_local_skill() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("skill install --path .codex/skills/gitbutler")
        .assert()
        .success();
    std::fs::write(
        env.projects_root().join(".codex/skills/gitbutler/SKILL.md"),
        "---\nname: but\nversion: old\n---\n",
    )
    .unwrap();

    // The staleness notice tells agents to run exactly this command; it must
    // converge instead of failing (it used to bail about a relative --path the
    // agent never passed when the repository was discovered from a relative
    // working directory).
    env.but("skill check --update")
        .env("AI_AGENT", "codex")
        .assert()
        .success();

    // The local copy is current again, so the notice stops.
    let output = env
        .but("alias list")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias list runs");
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("AGENT ACTION REQUIRED"),
        "the suggested remediation must clear the notice"
    );
}

#[test]
fn agent_skill_notice_repairs_another_agents_stale_global_skill() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.but("skill install")
        .env("AI_AGENT", "codex")
        .assert()
        .success();
    env.but("skill install")
        .env("AI_AGENT", "claude-code")
        .assert()
        .success();

    let claude_skill_path = env.home_dir().join(".claude/skills/gitbutler/SKILL.md");
    let expected = std::fs::read_to_string(&claude_skill_path).unwrap();
    std::fs::write(&claude_skill_path, "---\nname: but\nversion: old\n---\n").unwrap();

    let output = env
        .but("alias list")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias list runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("was out of date and was updated")
            && !stdout.contains("but skill check --update"),
        "another agent's stale global skill should be repaired before a read-only command, got: {stdout}"
    );
    assert_eq!(
        std::fs::read_to_string(&claude_skill_path).unwrap(),
        expected,
        "the detected agent should refresh other agents' global GitButler skill installations"
    );
}

#[test]
fn unrelated_update_failure_does_not_hide_missing_skill_hint() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.but("skill install")
        .env("AI_AGENT", "claude-code")
        .assert()
        .success();

    let claude_skill_dir = env.home_dir().join(".claude/skills/gitbutler");
    std::fs::write(
        claude_skill_dir.join("SKILL.md"),
        "---\nname: but\nversion: old\n---\n",
    )
    .unwrap();
    let concepts_path = claude_skill_dir.join("references/concepts.md");
    std::fs::remove_file(&concepts_path).unwrap();
    std::fs::create_dir(&concepts_path).unwrap();

    let output = env
        .but("alias list")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias list runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Install the GitButler skill before continuing")
            && !stdout.contains("auto-update failed"),
        "another agent's update failure must not hide the caller's missing skill hint, got: {stdout}"
    );

    env.but("skill install")
        .env("AI_AGENT", "codex")
        .assert()
        .success();
    let codex_skill_path = env.home_dir().join(".codex/skills/gitbutler/SKILL.md");
    let expected = std::fs::read_to_string(&codex_skill_path).unwrap();
    std::fs::write(&codex_skill_path, "---\nname: but\nversion: old\n---\n").unwrap();

    let output = env
        .but("alias list")
        .env("AI_AGENT", "codex")
        .output()
        .expect("alias list runs");
    assert_eq!(
        std::fs::read_to_string(&codex_skill_path).unwrap(),
        expected,
        "a failed Claude update should not prevent a later Codex update"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("auto-update failed"),
        "an unrelated failure should still be reported after other updates finish, got: {stdout}"
    );

    let output = env
        .but("alias list")
        .env("AI_AGENT", "claude-code")
        .output()
        .expect("alias list runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("auto-update failed"),
        "an installed caller's update failure should retain its actionable details, got: {stdout}"
    );
}

#[test]
fn skill_install_path_outside_repo_requires_global() {
    let env = Sandbox::empty();
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
}

#[test]
fn skill_install_absolute_path_outside_repo_does_not_require_global() -> anyhow::Result<()> {
    let env = Sandbox::empty();
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
    let paths = json
        .get("paths")
        .and_then(|v| v.as_array())
        .expect("paths array should be present");
    assert_eq!(
        paths.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
        vec![Some(expected_path.as_str())]
    );

    Ok(())
}

#[test]
fn skill_install_explicit_path_does_not_claim_the_agent_will_load_it() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    let install_dir = env.projects_root().join("agent-skill-install");

    let output = env
        .but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "agent", "--global"])
        .arg("--path")
        .arg(&install_dir)
        .env("AI_AGENT", "codex")
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
    assert!(
        !stdout.contains("To use it in this session"),
        "the read-it-now hint is for detected agent callers only, got: {stdout}"
    );

    Ok(())
}

#[test]
fn skill_check_detects_agent_skills_installation_in_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
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
fn skill_check_marks_an_incomplete_bundle_outdated() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");
    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success();
    std::fs::remove_file(
        env.projects_root()
            .join(&install_path)
            .join("references/concepts.md"),
    )?;

    let output = env
        .but("skill check --local --format json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(
        json.get("outdated_count").and_then(|value| value.as_u64()),
        Some(1)
    );
    Ok(())
}

#[test]
fn skill_install_detect_finds_agent_skills_installation_in_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
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
    let paths = json
        .get("paths")
        .and_then(|value| value.as_array())
        .expect("paths array should be present");
    assert!(
        paths
            .iter()
            .filter_map(|value| value.as_str())
            .any(path_ends_with_gitbutler_agents_dir),
        "expected detect to reuse .agents/skills/gitbutler, got: {json:?}"
    );

    Ok(())
}

#[test]
fn skill_install_detect_updates_every_installation_in_scope() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

    // Two GitButler skills installed under different formats in the local scope.
    for agent_dir in [".agents", ".claude"] {
        env.but("")
            .arg("skill")
            .arg("install")
            .args(["--format", "json"])
            .arg("--path")
            .arg(relative_agent_skill_path(agent_dir))
            .allow_json()
            .assert()
            .success();
    }

    let output = env
        .but("skill install --format json --detect")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output)?;
    let paths = json
        .get("paths")
        .and_then(|value| value.as_array())
        .expect("paths array should be present");
    assert_eq!(
        paths.len(),
        2,
        "detect refreshes every install in the scope, got: {json:?}"
    );

    Ok(())
}

#[test]
fn skill_check_ignores_format_outside_its_scope() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

    // `.copilot/skills` is a global-only format. Installed inside a repo via an
    // explicit --path, a local-scope scan must not discover (and later overwrite)
    // it as a local install.
    let copilot_path = std::path::PathBuf::from(".copilot")
        .join("skills")
        .join("gitbutler");
    env.but("")
        .arg("skill")
        .arg("install")
        .args(["--format", "json"])
        .arg("--path")
        .arg(&copilot_path)
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
        !skills.iter().any(|skill| {
            skill
                .get("path")
                .and_then(|value| value.as_str())
                .is_some_and(|path| {
                    path.replace('\\', "/")
                        .ends_with(".copilot/skills/gitbutler")
                })
        }),
        "a global-only .copilot install must not be discovered in local scope, got: {skills:?}"
    );

    Ok(())
}

#[test]
fn skill_install_surfaces_non_repo_discovery_errors() -> anyhow::Result<()> {
    let env = Sandbox::empty();
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
