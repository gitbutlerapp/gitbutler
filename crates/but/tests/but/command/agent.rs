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
        policy.contains("For commit just/only/specific changes on a new branch"),
        "policy should include the selected-change fast path, got: {policy}"
    );
    assert!(
        policy.contains("For that fast path, after the commit succeeds, stop and summarize"),
        "policy should tell agents not to re-verify the fast path unnecessarily, got: {policy}"
    );
    assert!(
        policy.contains("Use the installed GitButler skill for command recipes and syntax"),
        "policy should point agents to the installed skill for command details, got: {policy}"
    );
    assert!(
        policy.contains("Mutation commands report their result without appending workspace status"),
        "policy should explain concise mutation output, got: {policy}"
    );
    assert!(
        policy.contains("Add `--status-after` only when the next step needs"),
        "policy should explain situational status opt-in, got: {policy}"
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
