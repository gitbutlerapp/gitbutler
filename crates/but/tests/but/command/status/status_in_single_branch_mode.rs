use snapbox::IntoData;

use crate::utils::Sandbox;

#[test]
fn status_with_main_and_origin_main_in_sync() {
    // Corresponds to but-graph's workspace::target_commit::main_and_origin_main_in_sync.
    let env = Sandbox::open_with_default_settings("single-branch-in-sync");

    // The local branch and its upstream point to the same commit, without a workspace.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    // since we're checked out to main and it is in sync with origin/main
    // we show an empty workspace
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 85efbe4 (common base, main, origin/main) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[test]
fn status_with_empty_feature_branch_at_target() {
    // Corresponds to but-graph's workspace::target_commit::empty_feature_branch_at_target_remains_visible.
    let env = Sandbox::open_with_default_settings("single-branch-in-sync");
    env.invoke_git("checkout -b feature");

    // The new feature branch points to the same commit as main and origin/main,
    // without GitButler setup or explicit target configuration.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 85efbe4 (HEAD -> feature, origin/main, main) M

"#]]
    );

    // Unlike main, an empty feature branch should remain visible as a place to commit work.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ at [feature] [HEAD] (no commits)
├╯
┊
┴ 85efbe4 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_with_pushed_feature_commits_above_main() {
    let env = Sandbox::open_with_default_settings("single-branch-pushed-feature");

    // Unlike main, a feature branch's pushed commits are still work above the integration target.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 2da2cac (HEAD -> feature, origin/feature) feature
* 85efbe4 (origin/main, main) M

"#]]
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ at [feature] [HEAD]
┊●   xto feature (no changes)
├╯
┊
┴ 85efbe4 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_with_stacked_rebased_remotes() {
    // Corresponds to but-graph's init::stacked_rebased_remotes, with origin/main as integration target.
    let env = Sandbox::open_with_default_settings("single-branch-stacked-rebased-remotes");

    // Both local branches retain work; their diverged remotes are not incoming changes to main.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 2779236 (origin/B) B
* 2928071 (origin/A) A
| * d69fe94 (HEAD -> B) B
| * 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B] [HEAD]
┊┊
┊╭┄┄ (upstream: on origin/B)
┊●   2779236 B (no changes)
┊-
┊●   ksy B (no changes)
┊│
┊├┄ h0 [A]
┊┊
┊╭┄┄ (upstream: on origin/A)
┊●   2928071 A (no changes)
┊-
┊●   zmq A (no changes)
├╯
┊
┴ 85efbe4 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_with_feature_at_target_tip_and_main_behind() {
    // Corresponds to but-graph's init::ad_hoc_branch_at_target_tip.
    let env = Sandbox::open_with_default_settings("single-branch-feature-at-target-tip");

    // Feature and origin/main share a tip, but the local integration branch is still behind.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 2da2cac (HEAD -> feature, origin/main) feature
* 85efbe4 (main) M

"#]]
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ at [feature] [HEAD] (no commits)
├╯
┊
┴ 2da2cac (common base, origin/main) 2000-01-02 feature

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_with_main_ahead_of_origin_main() {
    // Complements but-graph's init::main_advanced_remote_advanced with only local advancement.
    let env = Sandbox::open_with_default_settings("single-branch-ahead");

    // Only main has a commit beyond the shared base.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 6476471 (HEAD -> main) local
* 85efbe4 (origin/main) M

"#]]
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] [HEAD]
┊●   syn local (no changes)
├╯
┊
┴ 85efbe4 (common base, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_with_main_behind_origin_main() {
    // Corresponds to but-graph's init::only_remote_advanced (and its special-branch-name variant).
    let env = Sandbox::open_with_default_settings("single-branch-behind");

    // Only origin/main has a commit beyond the shared base; main remains checked out.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 31dec87 (origin/main) remote
* 85efbe4 (HEAD -> main) M

"#]]
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊● 31dec87 (upstream: origin/main) 1 new commit
├╯ 85efbe4 (common base, main) 2000-01-02 M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[test]
fn status_with_both_diverged() {
    // Corresponds to but-graph's init::main_advanced_remote_advanced.
    let env = Sandbox::open_with_default_settings("single-branch-diverged");

    // Each branch has a distinct commit beyond the shared base, without a workspace.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 6476471 (HEAD -> main) local
| * 31dec87 (origin/main) remote
|/  
* 85efbe4 M

"#]]
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] [HEAD]
┊●   syn local (no changes)
├╯
┊
┊● 31dec87 (upstream: origin/main) 1 new commit
├╯ 85efbe4 (common base) 2000-01-02 M

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);
}
