use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

fn assert_default_policy(policy: &str) {
    assert!(
        policy.lines().count() <= 18,
        "default policy should be at most 18 lines, got {}: {policy}",
        policy.lines().count()
    );
    assert!(
        policy.len() <= 1_800,
        "default policy should be at most 1,800 bytes, got {}: {policy}",
        policy.len()
    );
    assert!(
        policy.contains("<!-- gitbutler-agent-setup:start -->"),
        "policy should include the managed block start marker, got: {policy}"
    );
    assert!(
        policy.contains("otherwise modify another agent's work"),
        "policy should include multi-agent isolation guidance, got: {policy}"
    );
    assert!(
        policy.contains("Use the installed GitButler skill"),
        "policy should point agents to the installed skill for command details, got: {policy}"
    );
    assert!(
        policy.contains(
            "If `but` reports `Setup required`, use plain Git equivalents and do not run `but setup` unless the user asks"
        ),
        "policy should explain the safe setup fallback, got: {policy}"
    );
    assert!(
        policy.contains("Use a dedicated branch for each agent session")
            && !policy.contains("dedicated GitButler branch"),
        "policy should keep the session branch rule valid under the plain-Git fallback, got: {policy}"
    );
    assert!(
        policy.contains("Fold small follow-up fixes into the unpublished commit they belong to")
            && policy
                .contains("ask before rewriting pushed, reviewed, shared, or ambiguous history"),
        "policy should include the concise fold-fixes preference, got: {policy}"
    );
    assert!(
        policy.contains("Suggest splitting unrelated changes into separate commits."),
        "policy should include the concise split preference, got: {policy}"
    );
    for old_recipe in [
        "For commit just/only/specific changes on a new branch",
        "For that fast path, after the commit succeeds",
        "Mutation commands report their result without appending workspace status",
        "Add `--status-after` only when the next step needs",
        "amend an unpublished local commit",
        "If one file contains unrelated changes, split them by hunk",
    ] {
        assert!(
            !policy.contains(old_recipe),
            "default policy should omit old recipe {old_recipe}, got: {policy}"
        );
    }
    assert!(
        policy.contains("<!-- gitbutler-agent-setup:end -->"),
        "policy should include the managed block end marker, got: {policy}"
    );
}

#[test]
fn agent_setup_print_outputs_default_managed_policy() {
    let env = Sandbox::empty();

    let output = env
        .but("agent setup --print")
        .assert()
        .success()
        .stderr_eq(str![[]])
        .get_output()
        .stdout
        .clone();
    let stdout = std::str::from_utf8(&output).unwrap();

    assert_default_policy(stdout);
}

#[test]
fn agent_setup_print_json_outputs_policy_field() {
    let env = Sandbox::empty();

    let output = env
        .but("--json agent setup --print")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![[]])
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let policy = json
        .get("policy")
        .and_then(|value| value.as_str())
        .expect("JSON output should include a string policy field");

    assert_default_policy(policy);
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

#[test]
fn agent_uninstall_removes_skills_and_gitbutler_sections_only() {
    let env = Sandbox::empty();
    env.invoke_git("init");
    let home = env.home_dir();
    let repo = env.projects_root();
    let block = "<!-- gitbutler-agent-setup:start -->\n## Version control\n\n- Use GitButler (`but`).\n<!-- gitbutler-agent-setup:end -->\n";
    let write = |path: &std::path::Path, content: &str| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    };

    env.but("skill install").as_agent().assert().success();
    env.but("skill install --path .agents/skills/gitbutler")
        .assert()
        .success();
    write(
        &home.join(".claude/skills/other/SKILL.md"),
        "---\nname: other\n---\n",
    );
    write(
        &home.join(".codex/AGENTS.md"),
        &format!("# Mine\n\nBe brief.\n\n{block}"),
    );
    write(&home.join(".claude/rules/gitbutler.md"), block);
    write(
        &home.join(".config/opencode/AGENTS.md"),
        "No GitButler here.\n",
    );
    write(&repo.join("AGENTS.md"), &format!("{block}\n# Project\n"));

    // Removal is listed per kind, with repository paths relative to the current directory.
    env.but("agent uninstall")
        .assert()
        .success()
        .stderr_eq(str![[]])
        .stdout_eq(str![[r#"
Found GitButler in your coding agents:

Skills
  • ~/.codex/skills/gitbutler
  • .agents/skills/gitbutler

GitButler section in
  • ~/.claude/rules/gitbutler.md
  • ~/.codex/AGENTS.md
  • AGENTS.md

✓ Removed.

"#]]);

    assert!(
        !home.join(".codex/skills/gitbutler").exists()
            && !repo.join(".agents/skills/gitbutler").exists(),
        "GitButler skill folders are removed"
    );
    assert!(
        home.join(".claude/skills/other/SKILL.md").exists(),
        "a skill without `name: but` is not GitButler's"
    );
    assert!(
        !home.join(".claude/rules/gitbutler.md").exists(),
        "a file that only held the GitButler section is deleted"
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".codex/AGENTS.md")).unwrap(),
        "# Mine\n\nBe brief.\n",
        "text before the section is kept"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("AGENTS.md")).unwrap(),
        "# Project\n",
        "text after the section is kept"
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".config/opencode/AGENTS.md")).unwrap(),
        "No GitButler here.\n",
        "a file without the section is not touched"
    );

    // Everything discovery can find was removed.
    env.but("agent uninstall")
        .assert()
        .success()
        .stdout_eq(str![[r#"
No GitButler skills or agent instructions found.

"#]]);
}
