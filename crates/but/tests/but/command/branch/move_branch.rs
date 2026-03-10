use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn move_branch_by_name_to_the_top_of_another() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   49b2841 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | 3842fc0 (C) add C
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch A on top of C
    env.but("branch move A C")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'A' on top of 'C'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   afff88e add A  
┊│
┊├┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn move_branch_by_cli_id_to_the_top_of_another() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   49b2841 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | 3842fc0 (C) add C
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch A on top of C
    env.but("branch move g0 h0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'A' on top of 'C'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   afff88e add A  
┊│
┊├┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn move_branch_by_cli_id_to_the_middle_of_another() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   49b2841 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | 3842fc0 (C) add C
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch A on top of B
    env.but("branch move g0 i0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'A' on top of 'B'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [C]  
┊●   c946b0e add C  
┊│
┊├┄h0 [A]  
┊●   8f3ad84 add A  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn move_branch_by_cli_id_from_the_bottom_to_the_top_of_another() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   49b2841 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | 3842fc0 (C) add C
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch B on top of A
    env.but("branch move i0 g0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'B' on top of 'A'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [B]  
┊●   b40d58b add B  
┊│
┊├┄h0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄i0 [C]  
┊●   31e83cd add C  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn reorder_branch_within_stack() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   49b2841 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | 3842fc0 (C) add C
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [C]  
┊●   3842fc0 add C  
┊│
┊├┄i0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch B on top of C
    env.but("branch move i0 h0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'B' on top of 'C'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [B]  
┊●   958528a add B  
┊│
┊├┄i0 [C]  
┊●   31e83cd add C  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn move_empty_branch_to_top_of_another_stack() -> anyhow::Result<()> {
    let env =
        Sandbox::open_or_init_scenario_with_target_and_default_settings("two-stacks-one-empty")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   802f604 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, B) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [B] (no commits) 
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch B on top of A
    env.but("branch move h0 g0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'B' on top of 'A'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [B] (no commits) 
┊│
┊├┄h0 [A]  
┊●   9477ae7 add A  
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}

#[test]
fn move_branch_on_top_of_empty_branch() -> anyhow::Result<()> {
    let env =
        Sandbox::open_or_init_scenario_with_target_and_default_settings("two-stacks-one-empty")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   802f604 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, B) add M
    ");

    env.setup_metadata(&["A", "B"])?;
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [B] (no commits) 
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    // Move branch B on top of A
    env.but("branch move g0 h0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved branch 'A' on top of 'B'.

"#]])
        .stderr_eq(str![""]);

    // Check that the operation succeeded.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   9477ae7 add A  
┊│
┊├┄h0 [B] (no commits) 
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
    Ok(())
}
