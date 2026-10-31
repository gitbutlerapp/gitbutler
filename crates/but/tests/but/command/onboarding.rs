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

// Branches made from origin/main, like with `git worktree add -b <name> <path> origin/main`, track
// it too, but main is named as the branch on the remote, so it's the one to migrate to.
#[cfg(feature = "legacy")]
#[test]
fn migration_to_local_target_ref_picks_tracking_branch_named_as_on_remote() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    // you cannot set the target ref to origin/main with `but config target origin/main`
    // so to emulate a legacy project we have to write the git config values directly
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/origin/main");
    let target_commit_id = env.invoke_git("rev-parse origin/main");
    env.invoke_git(&format!(
        "config gitbutler.project.targetCommitId {target_commit_id}"
    ));
    env.invoke_git("config gitbutler.project.portedMeta true");

    // found before main, as it sorts first
    env.invoke_git("branch --track agent-worktree origin/main");

    // main is the target, so it isn't listed as a branch in the workspace
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
fn migration_from_virtual_branches_toml_picks_tracking_branch_named_as_on_remote() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

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

    // found before main, as it sorts first
    env.invoke_git("branch --track agent-worktree origin/main");

    // main is the target, so it isn't listed as a branch in the workspace
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_eq!(
        env.invoke_git("config gitbutler.project.targetRef"),
        "refs/heads/main",
        "porting picks the branch named as on the remote over another tracking it"
    );
}

/// Make `trunk` and `agent-worktree` track `origin/main`, so none of the branches tracking it is
/// named as the branch on the remote.
#[cfg(feature = "legacy")]
fn tracking_branches_not_named_as_on_remote(env: &Sandbox) {
    env.invoke_git("branch -m main trunk");
    env.invoke_git("branch --track agent-worktree origin/main");
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

    tracking_branches_not_named_as_on_remote(&env);

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Multiple local branches track origin/main

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target trunk").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, origin/main) 2000-01-02 M

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

    tracking_branches_not_named_as_on_remote(&env);

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Multiple local branches track origin/main

Hint: Choose a local target branch with `but config target <branch>`

"#]]);

    env.but("config target trunk").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, origin/main) 2000-01-02 M

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
Error: GitButler now requires the target to be a local branch, but no local branch tracks your current target, origin/4.7

Hint: Make a local branch track origin/4.7 with `git branch --set-upstream-to=origin/4.7 <branch>`, or choose a local target branch with `but config target <branch>`

"#]]);

    // showing the target says it needs choosing, rather than that there is none
    env.but("config target")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: GitButler now requires the target to be a local branch, but no local branch tracks your current target, origin/4.7

Hint: Make a local branch track origin/4.7 with `git branch --set-upstream-to=origin/4.7 <branch>`, or choose a local target branch with `but config target <branch>`

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
Error: GitButler now requires the target to be a local branch, but no local branch tracks your current target, origin/4.7

Hint: Make a local branch track origin/4.7 with `git branch --set-upstream-to=origin/4.7 <branch>`, or choose a local target branch with `but config target <branch>`

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

// A fork checkout from before targets were local, where `upstream` is the project and `origin` is
// the user's fork that fell behind it.
//
// - Legacy target is `upstream/main`
// - `main` tracks the `origin/main`
//
// So no local branch tracks the legacy target. Following the hint to make `main` track it keeps
// pulls coming from the project, as they did before the migration.
//
// regression test for the fork follow-up to https://github.com/gitbutlerapp/gitbutler/pull/16437#issuecomment-6340526957
#[cfg(feature = "legacy")]
#[test]
fn migration_with_local_branch_tracking_fork_hints_to_track_legacy_target() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    let upstream = env.app_data_dir().join("upstream.git");
    let fork = env.app_data_dir().join("fork.git");
    let other = env.app_data_dir().join("other");
    let (upstream, fork, other) = (upstream.display(), fork.display(), other.display());
    // the fork is never synced while the project moves on
    env.invoke_bash(format!(
        "git clone -q --bare . {upstream} && git clone -q --bare . {fork} \
         && git clone -q {upstream} {other} && cd {other} \
         && echo upstream > upstream.txt && git add upstream.txt \
         && git commit -qm 'upstream work' && git push -q origin main"
    ));
    // local main keeps tracking origin/main, the fork
    env.invoke_bash(format!(
        "git remote set-url origin {fork} && git remote add upstream {upstream} \
         && git fetch -q origin && git fetch -q upstream"
    ));

    env.invoke_git("config gitbutler.project.targetRef refs/remotes/upstream/main");
    let target_commit_id = env.invoke_git("rev-parse upstream/main");
    env.invoke_git(&format!(
        "config gitbutler.project.targetCommitId {target_commit_id}"
    ));
    env.invoke_git("config gitbutler.project.pushRemote origin");
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: GitButler now requires the target to be a local branch, but no local branch tracks your current target, upstream/main

Hint: Make a local branch track upstream/main with `git branch --set-upstream-to=upstream/main <branch>`, or choose a local target branch with `but config target <branch>`

"#]]);

    // run the command the hint suggests
    env.invoke_git("branch --set-upstream-to=upstream/main main");

    // main is now the only branch tracking the legacy target, so it becomes the target by itself
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊● cb1cbd8 (upstream: upstream/main) 1 new commit
├╯ b1540e5 (common base, main, HEAD) 2000-01-02 M

Hint: upstream/main moved ahead; run `but pull` to update the workspace
Hint: run `but branch new` to create a new branch to work on

"#]]);

    // the project moves on again after the migration
    env.invoke_bash(format!(
        "cd {other} && echo more > more.txt && git add more.txt \
         && git commit -qm 'more upstream work' && git push -q origin main"
    ));
    env.but("pull")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Found 2 upstream commits on upstream/main
   dc682ed more upstream work
   cb1cbd8 upstream work

Rebase successful

Summary
────────

To undo this operation:
  Run `but undo`

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse main"),
        env.invoke_git("rev-parse upstream/main"),
        "pulling integrates the project, like the legacy target did"
    );
    assert_eq!(
        env.invoke_git("config gitbutler.project.targetRef"),
        "refs/heads/main",
        "the local branch tracking the legacy target replaces it"
    );
}

// An older GitButler stored the target origin/main for a main pushed without `-u`, so nothing
// tracks it. main is named as it and tracks nothing else, so it's made to track it.
#[cfg(feature = "legacy")]
#[test]
fn migration_makes_untracked_branch_named_as_legacy_target_track_it() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("branch --unset-upstream main");
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/origin/main");
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
        "origin/main",
        "pulls go on coming from the legacy target"
    );

    env.invoke_git("branch --unset-upstream main");
    env.but("status").assert().success();
    assert_eq!(
        env.invoke_git("config --default none branch.main.merge"),
        "none",
        "main replaced the legacy target once, so what the user sets it to track afterwards stays"
    );
}

#[cfg(feature = "legacy")]
#[test]
fn migration_creates_missing_branch_named_as_legacy_target() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_bash("git checkout -q -b feature && git branch -q -D main");
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/origin/main");
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ at [feature] [HEAD] (no commits)
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
        "origin/main",
        "main is created tracking the legacy target, as `git checkout main` would"
    );
    assert_eq!(
        env.invoke_git("rev-parse main"),
        env.invoke_git("rev-parse origin/main"),
        "main starts where the legacy target is"
    );
}

// main tracking a local branch has an upstream that no refspec maps, which is still an upstream.
#[cfg(feature = "legacy")]
#[test]
fn migration_leaves_branch_tracking_a_local_branch_alone() {
    // a legacy target a remote fetches, and one no remote fetches
    for (setup, target) in [
        ("true", "origin/main"),
        (
            "git config remote.gb-local.url . && git update-ref refs/remotes/gb-local/main main",
            "gb-local/main",
        ),
    ] {
        let env = Sandbox::open_with_default_settings("single-branch-mode");
        env.invoke_bash(format!(
            "{setup} && git branch -q trunk && git branch -q --set-upstream-to=trunk main"
        ));
        let target_ref = format!("refs/remotes/{target}");
        env.invoke_git(&format!("config gitbutler.project.targetRef {target_ref}"));
        env.invoke_git("config gitbutler.project.portedMeta true");

        env.but("status")
            .assert()
            .failure()
            .stderr_eq(snapbox::str![[r#"
Error: GitButler now requires the target to be a local branch, but no local branch tracks your current target, [..]

Hint: Make a local branch track [..] with `git branch --set-upstream-to=[..] <branch>`, or choose a local target branch with `but config target <branch>`

"#]]);
        assert_eq!(
            env.invoke_git("rev-parse --abbrev-ref main@{upstream}"),
            "trunk",
            "{target}: pulls into main go on coming from where the user set"
        );
        assert_eq!(
            env.invoke_git("config gitbutler.project.targetRef"),
            target_ref,
            "{target}: the legacy target stays for the user to choose"
        );
    }
}

// An older GitButler gave a repository without a remote a gb-local remote with only a URL, and
// the target gb-local/main. That remote fetches nothing, so main becomes the target itself.
#[cfg(feature = "legacy")]
#[test]
fn migration_of_gb_local_target_picks_branch_named_as_it() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    env.invoke_bash(
        "git config remote.gb-local.url . && git update-ref refs/remotes/gb-local/main main",
    );
    env.invoke_git("config gitbutler.project.targetRef refs/remotes/gb-local/main");
    env.invoke_git("config gitbutler.project.pushRemote gb-local");
    env.invoke_git("config gitbutler.project.portedMeta true");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 85efbe4 (common base, main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
    assert_eq!(
        env.invoke_git("config gitbutler.project.targetRef"),
        "refs/heads/main",
        "the local branch replaces the legacy target"
    );
}
