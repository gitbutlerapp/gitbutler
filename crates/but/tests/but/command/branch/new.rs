use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    env.but("branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch my-feature

"#]]);

    env.but("branch new --anchor 9477ae7 my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch my-anchored-feature stacked on [..]

"#]]);
    Ok(())
}

#[test]
fn rejects_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_normalizes_to_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new HEAD-")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD-'

Invalid branch name: Could not turn "HEAD-" into a valid reference name

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_normalizes_to_something_else_and_suggests_alternative() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new 'my branch'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'my branch'

Invalid branch name

Hint: Try 'my-branch' instead

"#]]);

    Ok(())
}

#[test]
fn rejects_branch_name_already_applied_in_workspace() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' is already applied

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_exists_outside_workspace() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.but("unapply A").assert().success();

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' exists but is not applied

"#]]);

    Ok(())
}

#[test]
fn handles_adding_branch_anchored_on_branch_shared_by_other_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new -a A first-stack").assert().success();

    env.file("first.txt", "first");
    env.but("commit -m first").assert().success();
    env.but("unapply first-stack").assert().success();

    env.but("apply A").assert().success();
    env.but("branch new -a A second-stack").assert().success();
    env.file("second.txt", "second");
    env.but("commit -m second").assert().success();

    env.but("apply first-stack").assert().success();

    // Precondition: Two stacks that share a branch
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄se [second-stack]
┊●   187de8a second
┊│
┊├┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄fi [first-stack]
┊●   8673c86 first
┊│
┊├┄h0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Act: add a branch anchored to the shared branch in _one_ of the stacks
    env.but("branch new -a h0 first-stack-middle")
        .assert()
        .success();

    // Assert: branch is added only to the targeted stack
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄se [second-stack]
┊●   187de8a second
┊│
┊├┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄fi [first-stack]
┊●   8673c86 first
┊│
┊├┄h0 [first-stack-middle] (no commits)
┊│
┊├┄i0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

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
    env.but("branch new --format json --anchor 9477ae7 my-anchored-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-anchored-feature",
  "anchor": "9477ae7"
}

"#]]);

    // TODO: on error
    // On error, we indicate this both by exit code and by json output to stdout
    // so tools would be able to detect it that way.
    Ok(())
}

#[test]
fn handles_path_prefix_collision() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    // As ref A already exists, A/new collides with A due to the need to create a directory called A
    env.but("branch new A/new/branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch name 'A/new/branch' collides with existing branch 'A'

"#]]);
}
