use crate::utils::Sandbox;

#[test]
fn local_alias_roundtrip_uses_repo_config() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.invoke_bash("git init repo");

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
