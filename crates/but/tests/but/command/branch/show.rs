use crate::utils::{CommandExt, Sandbox};

/// Show branch details for an applied branch using JSON output.
#[test]
fn show_lists_commits_ahead_for_applied_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let result = env
        .but("--json branch show applied-branch")
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied")?;
    env.setup_metadata(&["applied-branch"])?;

    let result = env
        .but("--json branch show applied-branch --check")
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
fn show_works_for_unapplied_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

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

    Ok(())
}

#[test]
fn supports_short_codes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch show g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

9477ae7 add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    Ok(())
}
