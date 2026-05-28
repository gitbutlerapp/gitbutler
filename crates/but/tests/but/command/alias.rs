use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn local_alias_roundtrip_uses_repo_config() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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

    Ok(())
}

#[test]
fn global_alias_roundtrip_uses_global_config() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
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

    Ok(())
}

#[test]
fn can_invoke_alias() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("alias add b branch").assert().success();

    env.but("b")
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[]]);

    Ok(())
}

#[test]
fn can_invoke_alias_with_root_flags() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("alias add b branch").assert().success();

    env.but("-t b")
        .assert()
        .stdout_eq(str![[r#"
Applied branches
active  ✓ *A          26y ago    author

"#]])
        .stderr_eq(str![[r#"
INFO [..]
INFO [..]
INFO [..]
"#]]);

    Ok(())
}

#[cfg(unix)]
#[test]
/// We allow aliases to shadow external commands. This is mostly for the sake of simplicity. There's
/// no particular security concern with this as aliases can only be defined in trusted files, so
/// there's no risk that you'd for example clone a repository with a malicious alias (as is the case
/// with Cargo, see https://github.com/rust-lang/cargo/issues/10049).
fn can_invoke_alias_that_shadows_external_command() -> anyhow::Result<()> {
    use std::{fs, os::unix::fs::PermissionsExt};

    use tempfile::tempdir;

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

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
