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
