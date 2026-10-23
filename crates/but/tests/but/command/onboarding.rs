use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn first_run_shows_metrics_message() {
    let env = Sandbox::empty();

    // The sandbox sets onboarding_complete: true by default to avoid polluting other tests.
    // Delete the settings file to simulate a fresh install (will be recreated with defaults).
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path).unwrap();

    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.

"#]]);
}

#[test]
fn second_run_is_silent() {
    let env = Sandbox::empty();

    // The sandbox sets onboarding_complete: true by default.
    // Delete the settings file to simulate a fresh install.
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path).unwrap();

    // First run shows the message and sets onboarding_complete
    env.but("onboarding").assert().success().stdout_eq(snapbox::str![[r#"
GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.

"#]]);

    // Second run should be completely silent (onboarding_complete is now true)
    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);
}

#[test]
fn json_mode_is_silent_but_marks_complete() {
    let env = Sandbox::empty();

    // The sandbox sets onboarding_complete: true by default.
    // Delete the settings file to simulate a fresh install.
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path).unwrap();

    // JSON mode should produce no output even on first run
    env.but("--json onboarding")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);

    // But it should still mark onboarding as complete, so human mode is also silent now
    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);
}

#[cfg(feature = "legacy")]
#[test]
fn migration_to_local_target_ref_with_multiple_tracking_branches() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    // you cannot set the target ref to origin/main with `but config target origin/main`
    // so to emulate a legacy project we have to write the git config values directly
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/origin/main");
    let target_commit_id = env.invoke_git("rev-parse origin/main");
    env.invoke_git(&format!(
        "config gitbutler.project.targetCommitId {target_commit_id}"
    ));
    env.invoke_git("config gitbutler.project.portedMeta true");

    // create a second branch that also tracks origin/main
    env.invoke_git("branch --track main-tracking origin/main");

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Multiple local branches track origin/main

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target main").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn migration_from_virtual_branches_toml_with_multiple_tracking_branches() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    // projects that were never ported to git config still have their target in
    // virtual_branches.toml, as a remote branch
    let target_commit_id = env.invoke_git("rev-parse origin/main");
    let gitbutler_dir = env.projects_root().join(".git/gitbutler");
    std::fs::create_dir_all(&gitbutler_dir).unwrap();
    std::fs::write(
        gitbutler_dir.join("virtual_branches.toml"),
        format!(
            "[default_target]\nremoteName = 'origin'\nbranchName = 'main'\nsha = '{target_commit_id}'\n"
        ),
    )
    .unwrap();

    // create a second branch that also tracks origin/main
    env.invoke_git("branch --track main-tracking origin/main");

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Multiple local branches track origin/main

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target main").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

/// Make a local `4.7` that tracks `mycompany/4.7`, so no local branch tracks `origin/4.7`.
#[cfg(feature = "legacy")]
fn local_branch_tracking_another_remote(env: &Sandbox) {
    env.invoke_git("branch -m main 4.7");
    env.invoke_git("update-ref refs/remotes/origin/4.7 refs/heads/4.7");
    let remote_url = env.invoke_git("remote get-url origin");
    env.invoke_git(&format!("remote add mycompany {remote_url}"));
    env.invoke_git("update-ref refs/remotes/mycompany/4.7 refs/heads/4.7");
    env.invoke_git("branch --set-upstream-to=mycompany/4.7 4.7");
}

// regression test for https://github.com/gitbutlerapp/gitbutler/pull/16437#issuecomment-6340526957
#[cfg(feature = "legacy")]
#[test]
fn migration_to_local_target_ref_without_tracking_branch() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    local_branch_tracking_another_remote(&env);

    // the legacy target names origin, but the only local counterpart tracks mycompany
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/origin/4.7");
    let target_commit_id = env.invoke_git("rev-parse origin/4.7");
    env.invoke_git(&format!(
        "config gitbutler.project.targetCommitId {target_commit_id}"
    ));
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No local branch tracks origin/4.7

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target 4.7").assert().success();

    assert_eq!(
        env.invoke_git("config gitbutler.project.targetRef"),
        "refs/heads/4.7",
        "the chosen local branch replaces the legacy target"
    );
    assert_eq!(
        env.invoke_git("config gitbutler.project.targetCommitId"),
        target_commit_id,
        "choosing the local counterpart keeps the legacy cutoff"
    );

    // the upstream comes from the local branch's own config, not from the legacy target
    env.but("config target")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Used to determine common base to calculate commits unique to each branch (not yet integrated)

Target Branch:

  mycompany/4.7

  Remote: ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
  SHA:    85efbe4d5a663bff0ed8fb5fbc38a72be0592f55

New commits on the remote arrive via `but pull` — the target setting is not what makes the workspace current.
To point the workspace at a different branch entirely:
  but config target <branch_name>

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 85efbe4 (common base, 4.7, mycompany/4.7, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[cfg(feature = "legacy")]
#[test]
fn migration_from_virtual_branches_toml_without_tracking_branch() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    local_branch_tracking_another_remote(&env);

    let target_commit_id = env.invoke_git("rev-parse origin/4.7");
    let gitbutler_dir = env.projects_root().join(".git/gitbutler");
    std::fs::create_dir_all(&gitbutler_dir).unwrap();
    std::fs::write(
        gitbutler_dir.join("virtual_branches.toml"),
        format!(
            "[default_target]\nremoteName = 'origin'\nbranchName = '4.7'\nsha = '{target_commit_id}'\n"
        ),
    )
    .unwrap();

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No local branch tracks origin/4.7

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target 4.7").assert().success();

    assert_eq!(
        env.invoke_git("config gitbutler.project.targetRef"),
        "refs/heads/4.7",
        "the chosen local branch replaces the legacy target"
    );
    assert_eq!(
        env.invoke_git("config gitbutler.project.targetCommitId"),
        target_commit_id,
        "porting keeps the legacy cutoff until a target is chosen"
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 85efbe4 (common base, 4.7, mycompany/4.7, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}
