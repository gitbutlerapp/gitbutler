use crate::utils::Sandbox;

#[cfg(feature = "legacy")]
use snapbox::str;

#[test]
fn local_alias_roundtrip_uses_repo_config() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");

    env.invoke_git_fails(
        "-C repo config --local --get but.alias.st",
        "no local alias is set",
    );
    env.but("-C repo alias add st status").assert().success();
    assert_eq!(
        env.invoke_git("-C repo config --local --get but.alias.st"),
        "status"
    );

    env.but("-C repo alias remove st").assert().success();
    env.invoke_git_fails(
        "-C repo config --local --get but.alias.st",
        "local alias should be removed from repo config",
    );
}

#[test]
fn global_alias_roundtrip_uses_global_config() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");
    let global_config = env.projects_root().join("global.gitconfig");

    env.but("-C repo alias add st status --global")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();
    assert_eq!(
        env.invoke_git("config --file global.gitconfig --get but.alias.st"),
        "status"
    );
    env.invoke_git_fails(
        "-C repo config --local --get but.alias.st",
        "global alias should not touch the repo-local config",
    );

    env.but("-C repo alias remove st --global")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .assert()
        .success();
    env.invoke_git_fails(
        "config --file global.gitconfig --get but.alias.st",
        "global alias should be removed from the configured global file",
    );
}

#[cfg(feature = "legacy")]
#[test]
fn can_invoke_alias() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("alias add b branch").assert().success();

    env.but("b")
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[]]);
}

#[cfg(feature = "legacy")]
#[test]
fn can_invoke_alias_with_root_flags() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("alias add b branch").assert().success();

    let output = env.but("-t b").output()?;
    assert!(
        output.status.success(),
        "alias with root trace flag should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(
        stdout,
        "Applied branches\nactive  ✓ *A          26y ago    author\n"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.lines().any(|line| line.starts_with("INFO ")),
        "trace flag should emit INFO logs: {stderr}"
    );

    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn can_invoke_alias_with_root_flags_with_args() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("alias add b branch").assert().success();
    let log_file_path = env.projects_root().join("log_file.txt");

    env.but("-t --log-file")
        .arg(&log_file_path)
        .arg("b")
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[]]);

    let log_file_content = std::fs::read_to_string(&log_file_path)?;
    assert!(!log_file_content.is_empty(), "Log file must be non-empty");

    Ok(())
}

#[cfg(unix)]
#[cfg(feature = "legacy")]
#[test]
/// We allow aliases to shadow external commands. This is mostly for the sake of simplicity. There's
/// no particular security concern with this as aliases can only be defined in trusted files, so
/// there's no risk that you'd for example clone a repository with a malicious alias (as is the case
/// with Cargo, see https://github.com/rust-lang/cargo/issues/10049).
fn can_invoke_alias_that_shadows_external_command() -> anyhow::Result<()> {
    use std::{fs, os::unix::fs::PermissionsExt};

    use tempfile::tempdir;

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let bin = tempdir()?;
    let helper = bin.path().join("but-b");
    fs::write(&helper, "#!/bin/sh\necho 'called but-b'\n")?;
    let mut perms = fs::metadata(&helper)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms)?;

    let path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{path}", bin.path().display());

    env.but("b")
        .env("PATH", &new_path)
        .assert()
        .stdout_eq(str![[r#"
called but-b

"#]])
        .stderr_eq(str![[]]);

    // Alias should shadow external command
    env.but("alias add b branch").assert().success();
    env.but("b")
        .env("PATH", &new_path)
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[]]);

    Ok(())
}

#[cfg(unix)]
#[cfg(feature = "legacy")]
#[test]
fn alias_expansion_failure_falls_back_to_original_args() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");

    env.but("alias add bad").arg("'").assert().success();

    env.but("bad")
        .assert()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Failed to expand alias 'bad': missing closing quote
Skipping alias expansion
error: unrecognized subcommand 'bad'

  tip: a similar subcommand exists: 'onboarding'

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);
}

#[cfg(unix)]
#[cfg(feature = "legacy")]
#[test]
/// `but alias add branch help` rejects aliases that conflict with known commands, but a user
/// can still write such an alias into git config directly. Known subcommands must not expand!
///
/// There are multiple layers of protection in the application against this. The parsing should make
/// it impossible as known subcommands should not be parsed as external commands, and the alias
/// expansion itself should bail if a known subcommand is passed in.
fn alias_of_known_subcommand_is_not_expanded() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.invoke_git("config --local but.alias.branch help");

    env.but("branch")
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[]]);

    Ok(())
}
