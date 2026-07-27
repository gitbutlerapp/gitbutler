use snapbox::str;

use crate::{
    command::util::{branch_commit_cli_ids, status_json_with_files as status_json},
    utils::Sandbox,
};

fn uncommitted_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
    status["uncommittedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

fn branch_commits_contain_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .filter(|branch| branch["name"].as_str().unwrap() == branch_name)
        .flat_map(|branch| branch["commits"].as_array().unwrap().iter())
        .flat_map(|commit| commit["changes"].as_array().unwrap().iter())
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

#[test]
fn amend_rejects_dependency_changes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    // Commit `first` to branch foo and an unrelated file to branch bar.
    env.file("first", "Some text");
    env.but("commit -m 'add first' -b foo").assert().success();
    env.file("second", "Other text");
    env.but("commit -m 'add second' -b bar").assert().success();

    // Change `first` (which depends on foo) and try to amend it into bar's
    // commit. The squash internals reject the operation atomically.
    env.file("first", "changes");
    let status = status_json(&env)?;
    let bar_commit_cli_id = branch_commit_cli_ids(&status, "bar")[0].clone();
    env.but(format!("amend first --target {bar_commit_cli_id}"))
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Couldn't squash all changes

"#]]);

    let after = status_json(&env)?;
    assert!(
        uncommitted_contains_file(&after, "first"),
        "a rejected amend must leave its source uncommitted"
    );
    assert!(
        !branch_commits_contain_file(&after, "bar", "first"),
        "a rejected amend must not modify the target branch"
    );

    Ok(())
}

#[test]
fn amend_accepts_multiple_uncommitted_changes() {
    assert_multiple_amend(|target_cli_id| {
        format!("amend one.txt two.txt --target {target_cli_id}")
    })
    .unwrap();
}

#[test]
fn amend_accepts_branch_target() {
    assert_multiple_amend(|_target_cli_id| "amend one.txt two.txt --target A".to_string()).unwrap();
}

fn assert_multiple_amend(args: impl FnOnce(&str) -> String) -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("one.txt", "one\n");
    env.file("two.txt", "two\n");
    env.file("three.txt", "three\n");

    let before = status_json(&env)?;
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(args(&target_cli_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended [..] to create [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        !uncommitted_contains_file(&after, "one.txt"),
        "first amended file should no longer be uncommitted"
    );
    assert!(
        !uncommitted_contains_file(&after, "two.txt"),
        "second amended file should no longer be uncommitted"
    );
    assert!(
        uncommitted_contains_file(&after, "three.txt"),
        "unmentioned file should remain uncommitted"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "one.txt"),
        "first file should be amended into a commit on branch A"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "two.txt"),
        "second file should be amended into a commit on branch A"
    );

    Ok(())
}

#[test]
fn amend_rejects_merged_upstream_commit() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    let status = status_json(&env).unwrap();
    let source = status["uncommittedChanges"][0]["cliId"]
        .as_str()
        .unwrap()
        .to_string();

    // `nyq` is branch A's commit, whose content already landed on origin/main.
    env.but(format!("amend -t nyq {source}"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    // The escape hatch permits amending landed commits regardless.
    env.but(format!("amend -t nyq {source} --allow-merged"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended 756ee31 to create f18cbfd

"#]]);
}

#[test]
fn amend_rejects_landed_commit_in_partially_integrated_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "C"], "refs/heads/base")
        .unwrap();
    env.file("file.txt", "Some text");

    let status = status_json(&env).unwrap();
    let source = status["uncommittedChanges"][0]["cliId"]
        .as_str()
        .unwrap()
        .to_string();
    let landed_bottom = branch_commit_cli_ids(&status, "C")
        .pop()
        .expect("branch C has a commit");

    // Branch C at the bottom of the stack has landed while A on top is live;
    // the landed commit alone must be refused.
    env.but(format!("amend -t {landed_bottom} {source}"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit e5378e0 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}
