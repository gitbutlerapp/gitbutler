use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn error_when_stacking_only_branch_in_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    
    // Try to stack B (the only branch in its stack) onto A - should fail
    // This is the expected behavior since B is a single-branch stack
    env.but("stack B A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to stack branch. Cannot stack 'B' as it is the only named branch in its stack. Use 'but branch new --anchor A' to create a new branch stacked on 'A' instead.

"#]]);
    
    Ok(())
}

#[test]
fn error_when_source_not_found() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    
    env.setup_metadata(&["A"])?;
    
    env.but("stack nonexistent A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to stack branch. Source branch 'nonexistent' not found. If you just performed a Git operation, try running 'but status' to refresh.

"#]]);
    
    Ok(())
}

#[test]
fn error_when_target_not_found() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    
    env.setup_metadata(&["A"])?;
    
    env.but("stack A nonexistent")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to stack branch. Target branch 'nonexistent' not found. If you just performed a Git operation, try running 'but status' to refresh.

"#]]);
    
    Ok(())
}

#[test]
fn error_when_source_is_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    
    env.setup_metadata(&["A"])?;
    
    // Try to use a commit ID as source (should be a branch)
    env.but("stack 9477ae7 A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to stack branch. Source '9477ae7' must be a branch, but got a commit

"#]]);
    
    Ok(())
}

#[test]
fn error_when_same_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    
    env.setup_metadata(&["A"])?;
    
    env.but("stack A A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to stack branch. Source and target branches cannot be the same

"#]]);
    
    Ok(())
}
