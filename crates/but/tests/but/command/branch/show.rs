use crate::utils::{CommandExt, Sandbox};

/// Show branch details for an applied branch using JSON output.
#[test]
fn show_lists_commits_ahead_for_applied_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied");
    env.setup_metadata(&["applied-branch"]);

    let result = env
        .but("--format json branch show applied-branch")
        .allow_json()
        .output()?;

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;

    assert_eq!(json["branch"], "applied-branch");
    assert_eq!(json["commitsAhead"], 1);
    assert_eq!(json["commits"].as_array().unwrap().len(), 1);
    assert_eq!(json["commits"][0]["message"], "add applied.txt");

    Ok(())
}

/// Report merge-check information for an applied branch using JSON output.
#[test]
fn show_check_reports_clean_merge_for_applied_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied");
    env.setup_metadata(&["applied-branch"]);

    let result = env
        .but("--format json branch show applied-branch --check")
        .allow_json()
        .output()?;

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;

    assert_eq!(json["branch"], "applied-branch");
    assert_eq!(json["mergeCheck"]["mergesCleanly"], true);
    assert_eq!(
        json["mergeCheck"]["conflictingFiles"],
        serde_json::json!([])
    );

    Ok(())
}

#[test]
fn show_works_for_unapplied_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch show A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("unapply A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unapplied stack with branches 'A' from workspace

"#]]);

    env.but("branch show A")
        .assert()
        .stderr_eq(snapbox::str![[""]])
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn supports_short_codes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch show g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn show_works_for_remote_only_branches() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.invoke_bash(
        r#"
git checkout origin/main
git commit -m 'Add remote feature' --allow-empty
git update-ref refs/remotes/origin/remote-feature HEAD
git checkout gitbutler/workspace
"#,
    );

    env.but("branch show origin/remote-feature")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: origin/remote-feature (1 commits ahead)

ba02e5f Add remote feature
    2000-01-02 00:00:00 by author
    0 files changed, 0 insertions, 0 deletions

"#]]);

    env.but("branch show remote-feature")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: remote-feature (1 commits ahead)

ba02e5f Add remote feature
    2000-01-02 00:00:00 by author
    0 files changed, 0 insertions, 0 deletions

"#]]);
}

#[test]
fn showing_branch_that_isnt_top_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted changes] (no changes)
┊
┊╭┄g0 [C]
┊●   aebb090 add C
┊│
┊├┄h0 [B]
┊●   582f37b add B
┊│
┊├┄i0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch show C")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: C (3 commits ahead)

aebb090 add C
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

582f37b add B
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("branch show B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: B (2 commits ahead)

582f37b add B
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("branch show A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn checking_merge_status_of_branch_that_isnt_top_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted changes] (no changes)
┊
┊╭┄g0 [C]
┊●   aebb090 add C
┊│
┊├┄h0 [B]
┊●   582f37b add B
┊│
┊├┄i0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch show C --check").assert().success();

    env.but("branch show B --check").assert().success();

    env.but("branch show A --check").assert().success();
}
