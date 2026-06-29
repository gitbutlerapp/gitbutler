use snapbox::str;

use crate::{
    command::util::{branch_commit_ids, status_json_with_files as status_json},
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
fn amend_reports_dependency_changes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    // Commit `first` to branch foo and an unrelated file to branch bar.
    env.file("first", "Some text");
    env.but("commit -m 'add first' -c foo").assert().success();
    env.file("second", "Other text");
    env.but("commit -m 'add second' -c bar").assert().success();

    // Change `first` (which depends on foo) and try to amend it into bar's
    // commit. It cannot land there, so amend should name the branch/commit it
    // depends on and suggest stacking bar onto foo.
    env.file("first", "changes");
    let status = status_json(&env)?;
    let bar_commit = branch_commit_ids(&status, "bar")[0].clone();
    env.but(format!("amend {bar_commit} --changes first"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended the only hunk in first in the uncommitted area → [..]
Note: 1 change could not be applied:
  first
    line 1 depends on foo ([..])

Hint: you can stack bar on top of foo to apply these changes:
  but move bar foo

"#]]);

    Ok(())
}

#[test]
fn amend_accepts_comma_separated_uncommitted_changes() {
    assert_multiple_amend(|target_commit| {
        format!("amend {target_commit} --changes one.txt,two.txt")
    })
    .unwrap();
}

#[test]
fn amend_legacy_form_still_accepts_comma_separated_uncommitted_changes() {
    assert_multiple_amend(|target_commit| format!("amend one.txt,two.txt {target_commit}"))
        .unwrap();
}

#[test]
fn amend_without_changes_prints_new_usage_hint() {
    let env = Sandbox::empty();

    env.but("amend c3")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Missing --changes <file-or-hunk>. Usage: but amend <commit> --changes <id>[,<id>]

"#]]);
}

#[test]
fn amend_with_changes_rejects_extra_positional() {
    let env = Sandbox::empty();

    env.but("amend c3 --changes a1 b2")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Unexpected extra argument 'b2'. Use comma-separated --changes values: but amend <commit> --changes <id>[,<id>]

"#]]);
}

#[test]
fn amend_with_changes_rejects_empty_change_ids() {
    let env = Sandbox::empty();

    env.but("amend c3 --changes a1,,b2")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Empty --changes value. Use comma-separated file or hunk IDs: but amend <commit> --changes <id>[,<id>]

"#]]);
}

fn assert_multiple_amend(args: impl FnOnce(&str) -> String) -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("one.txt", "one\n");
    env.file("two.txt", "two\n");
    env.file("three.txt", "three\n");

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(args(&target_commit))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended hunk(s) → [..]

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
