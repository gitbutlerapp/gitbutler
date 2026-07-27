use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn outputs_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

    env.but("branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch my-feature

"#]]);

    env.but("branch new --anchor tpm my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch my-anchored-feature stacked on [..]

"#]]);
}

#[test]
fn rejects_anchor_outside_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("branch new --anchor A new-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'A'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn rejects_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);
}

#[test]
fn rejects_name_that_normalizes_to_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new HEAD-")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD-'

Invalid branch name: Could not turn "HEAD-" into a valid reference name

"#]]);
}

#[test]
fn rejects_name_that_normalizes_to_something_else_and_suggests_alternative() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new 'my branch'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'my branch'

Invalid branch name

Hint: Try 'my-branch' instead

"#]]);
}

#[test]
fn rejects_branch_name_already_applied_in_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' is already applied

"#]]);
}

#[test]
fn rejects_name_that_exists_outside_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' exists but is not applied

"#]]);
}

#[test]
fn with_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

    // Test JSON output without anchor
    env.but("--format json branch new my-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);

    // Test JSON output with anchor
    env.but("branch new --format json --anchor tpm my-anchored-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-anchored-feature",
  "anchor": "tpm"
}

"#]]);

    // TODO: on error
    // On error, we indicate this both by exit code and by json output to stdout
    // so tools would be able to detect it that way.
}

#[test]
fn single_branch_outputs_created_branch_for_all_formats() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("one-fork");

    env.but("branch new human-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch human-feature

"#]]);

    env.but("branch new shell-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch shell-feature

"#]]);

    env.but("--format json branch new json-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "json-feature"
}

"#]]);

    let repo = env.open_repo();
    for branch_name in ["human-feature", "shell-feature", "json-feature"] {
        let reference_name = format!("refs/heads/{branch_name}");
        assert!(
            repo.try_find_reference(reference_name.as_str())?.is_some(),
            "single-branch creation writes the branch reference"
        );
    }
    assert!(
        repo.try_find_reference(but_core::WORKSPACE_REF_NAME)?
            .is_none(),
        "single-branch creation does not create a managed workspace reference"
    );
    Ok(())
}

#[test]
fn handles_path_prefix_collision() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // As ref A already exists, A/new collides with A due to the need to create a directory called A
    env.but("branch new A/new/branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch name 'A/new/branch' collides with existing branch 'A'

"#]]);
}

#[test]
fn creates_new_branches_on_top() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new one").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new two").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ tw [two] (no commits)
├╯
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
