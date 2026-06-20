use snapbox::str;

use crate::{
    command::util::{branch_commit_ids, status_json_with_files as status_json},
    utils::Sandbox,
};

fn unassigned_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
    status["unassignedChanges"]
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
fn amend_accepts_comma_separated_uncommitted_changes() -> anyhow::Result<()> {
    assert_multiple_amend(|target_commit| {
        format!("amend {target_commit} --changes one.txt,two.txt")
    })
}

#[test]
fn amend_legacy_form_still_accepts_comma_separated_uncommitted_changes() -> anyhow::Result<()> {
    assert_multiple_amend(|target_commit| format!("amend one.txt,two.txt {target_commit}"))
}

#[test]
fn amend_without_changes_prints_new_usage_hint() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("amend c3")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Missing --changes <file-or-hunk>. Usage: but amend <commit> --changes <id>[,<id>]

"#]]);

    Ok(())
}

#[test]
fn amend_with_changes_rejects_extra_positional() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("amend c3 --changes a1 b2")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Unexpected extra argument 'b2'. Use comma-separated --changes values: but amend <commit> --changes <id>[,<id>]

"#]]);

    Ok(())
}

#[test]
fn amend_with_changes_rejects_empty_change_ids() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("amend c3 --changes a1,,b2")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Empty --changes value. Use comma-separated file or hunk IDs: but amend <commit> --changes <id>[,<id>]

"#]]);

    Ok(())
}

fn assert_multiple_amend(args: impl FnOnce(&str) -> String) -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

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
        !unassigned_contains_file(&after, "one.txt"),
        "first amended file should no longer be unassigned"
    );
    assert!(
        !unassigned_contains_file(&after, "two.txt"),
        "second amended file should no longer be unassigned"
    );
    assert!(
        unassigned_contains_file(&after, "three.txt"),
        "unmentioned file should remain unassigned"
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
