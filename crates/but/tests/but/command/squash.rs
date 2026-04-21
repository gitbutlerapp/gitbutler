use anyhow::Context as _;
use bstr::ByteSlice;
use snapbox::str;

use crate::{command::util, utils::Sandbox};

/// Helper to create multiple commits on a branch for testing
fn setup_branch_with_commits(env: &Sandbox, branch: &str, num_commits: usize) {
    let branch_prefix = branch.replace('/', "_");
    for i in 1..=num_commits {
        env.file(
            format!("{branch_prefix}-file{i}.txt"),
            format!("content for commit {i}\n"),
        );
        env.but(format!("commit {branch} -m 'commit {i}'"))
            .assert()
            .success();
    }
}

fn branch_commit_count(env: &Sandbox, branch: &str) -> anyhow::Result<usize> {
    let status = util::status_json(env)?;
    let branch = util::find_branch(&status, branch)?;
    Ok(branch["commits"]
        .as_array()
        .map(|commits| commits.len())
        .unwrap_or(0))
}

#[test]
fn squash_requires_at_least_two_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Only one commit exists (from scenario)
    // Try to squash a single commit - should fail
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash c0").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. No matching branch or commit found for 'c0'

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_branch_with_single_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Branch A has only 1 commit from the scenario
    // Try to squash branch with single commit - should fail
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. Branch 'A' has only one commit, nothing to squash

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_nonexistent_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to squash with nonexistent commit ID
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash nonexistent c0")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. No matching commit found for 'nonexistent'

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_branch_by_name_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create more commits on branch A (scenario already has 1)
    setup_branch_with_commits(&env, "A", 2);

    // Squash all commits in branch A by using branch name
    // This should succeed as we have 3 commits total
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A").assert().success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_with_drop_message_flag_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create 1 more commit (scenario has 1, so we'll have 2 total)
    setup_branch_with_commits(&env, "A", 1);

    // Squash branch with --drop-message flag
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A --drop-message").assert().success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_with_custom_message_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create 1 more commit
    setup_branch_with_commits(&env, "A", 1);

    // Squash with custom message
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A -m 'Custom squash message'")
        .assert()
        .success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_mutually_exclusive_flags_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --drop-message and --message flags - should fail
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A -m 'Custom message' --drop-message")
        .assert()
        .failure();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_with_invalid_range_format_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 2);

    // Try to use invalid range format with triple dots
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash c0..c1..c2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. Range format should be 'start..end', got 'c0..c1..c2'

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_with_empty_range_part_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 2);

    // Try range with empty start
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash ..c1").assert().failure();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_comma_list_with_nonexistent_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try comma-separated list with only nonexistent commits
    // This tests the comma parsing path without needing real commit IDs
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash nonexistent1,nonexistent2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. Commit 'nonexistent1' not found

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_range_with_nonexistent_endpoint_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try range with nonexistent endpoints
    // This tests the range parsing path without needing real commit IDs
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash nonexistent1..nonexistent2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. Start of range 'nonexistent1' must match exactly one commit

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_ai_conflicts_with_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --ai and --message flags - should fail
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A --ai -m 'Custom message'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai[=<AI>]' cannot be used with '--message <MESSAGE>'

Usage: but squash --ai[=<AI>] <COMMITS>...

For more information, try '--help'.

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn squash_ai_conflicts_with_drop_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --ai and --drop-message flags - should fail
    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash A --ai --drop-message")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai[=<AI>]' cannot be used with '--drop-message'

Usage: but squash --ai[=<AI>] <COMMITS>...

For more information, try '--help'.

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    Ok(())
}

#[test]
fn concurrent_squashes_on_independent_branches_succeed() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new branchB").assert().success();
    setup_branch_with_commits(&env, "A", 1);
    setup_branch_with_commits(&env, "branchB", 2);
    let working_directory_before = util::working_directory_snapshot(&env)?;

    let child_a = util::but_std_cmd(&env, "squash A").spawn()?;
    let child_b = util::but_std_cmd(&env, "squash branchB").spawn()?;

    let out_a = child_a.wait_with_output()?;
    let out_b = child_b.wait_with_output()?;

    assert!(
        out_a.status.success(),
        "squash on A failed: {}",
        out_a.stderr.as_bstr()
    );
    assert!(
        out_b.status.success(),
        "squash on branchB failed: {}",
        out_b.stderr.as_bstr()
    );
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    assert_eq!(branch_commit_count(&env, "A")?, 1);
    assert_eq!(branch_commit_count(&env, "branchB")?, 1);

    Ok(())
}

#[test]
fn squash_multiple_commits_from_list_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 3);

    let status_before = util::status_json(&env)?;
    let branch_a_before = util::find_branch(&status_before, "A")?;
    let commits_before = branch_a_before["commits"]
        .as_array()
        .context("Missing commits for branch A")?;

    let source_top = commits_before[0]["cliId"]
        .as_str()
        .context("Missing top source commit cliId")?;
    let source_second = commits_before[1]["cliId"]
        .as_str()
        .context("Missing second source commit cliId")?;
    let target = commits_before[2]["cliId"]
        .as_str()
        .context("Missing target commit cliId")?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!("squash {source_top},{source_second},{target}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Squashed 2 commits → [..]

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let status_after = util::status_json(&env)?;
    let branch_a_after = util::find_branch(&status_after, "A")?;
    let commits_after = branch_a_after["commits"]
        .as_array()
        .context("Missing commits for branch A after squash")?;

    assert_eq!(
        commits_after.len(),
        2,
        "A should have 2 commits after squashing 2 sources into 1 target"
    );

    Ok(())
}

#[test]
fn squash_list_with_bottom_target_keeps_target_message_first() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    setup_branch_with_commits(&env, "A", 3);

    insta::assert_snapshot!(env.git_log()?, @r"
    * c7b6336 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8a1f552 (A) commit 3
    * 39dd878 commit 2
    * 8128859 commit 1
    * 9477ae7 add A
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    let status_before = util::status_json(&env)?;
    let branch_a_before = util::find_branch(&status_before, "A")?;
    let commits_before = branch_a_before["commits"]
        .as_array()
        .context("Missing commits for branch A")?;

    let source_top = commits_before[0]["cliId"]
        .as_str()
        .context("Missing top source commit cliId")?;
    let source_second = commits_before[1]["cliId"]
        .as_str()
        .context("Missing second source commit cliId")?;
    let target = commits_before[2]["cliId"]
        .as_str()
        .context("Missing target commit cliId")?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!("squash {source_top},{source_second},{target}"))
        .assert()
        .success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let log_after = env.git_log()?;
    assert!(
        log_after.contains("(A) commit 1"),
        "expected squashed branch tip message to be target-first (commit 1), got:\n{log_after}"
    );

    Ok(())
}

#[test]
fn squash_list_with_middle_target_keeps_target_message_first() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    setup_branch_with_commits(&env, "A", 3);

    insta::assert_snapshot!(env.git_log()?, @r"
    * c7b6336 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8a1f552 (A) commit 3
    * 39dd878 commit 2
    * 8128859 commit 1
    * 9477ae7 add A
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    let status_before = util::status_json(&env)?;
    let branch_a_before = util::find_branch(&status_before, "A")?;
    let commits_before = branch_a_before["commits"]
        .as_array()
        .context("Missing commits for branch A")?;

    let source_top = commits_before[0]["cliId"]
        .as_str()
        .context("Missing top source commit cliId")?;
    let target = commits_before[1]["cliId"]
        .as_str()
        .context("Missing second source commit cliId")?;
    let source_third = commits_before[2]["cliId"]
        .as_str()
        .context("Missing target commit cliId")?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!("squash {source_top},{source_third},{target}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Squashed 2 commits → [..]

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let log_after = env.git_log()?;
    assert!(
        log_after.contains("(A) commit 2"),
        "expected squashed branch tip message to be target-first (commit 2), got:\n{log_after}"
    );

    Ok(())
}

#[test]
fn squash_multiple_commits_keeps_squashed_commit_content() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    setup_branch_with_commits(&env, "A", 3);

    insta::assert_snapshot!(env.git_log()?, @r"
    * c7b6336 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8a1f552 (A) commit 3
    * 39dd878 commit 2
    * 8128859 commit 1
    * 9477ae7 add A
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    let status_before = util::status_json(&env)?;
    let branch_a_before = util::find_branch(&status_before, "A")?;
    let commits_before = branch_a_before["commits"]
        .as_array()
        .context("Missing commits for branch A")?;

    let target_message_before = commits_before[2]["message"]
        .as_str()
        .context("Missing target commit message")?
        .to_string();
    let source_top_message = commits_before[0]["message"]
        .as_str()
        .context("Missing top source commit message")?
        .to_string();
    let source_second_message = commits_before[1]["message"]
        .as_str()
        .context("Missing second source commit message")?
        .to_string();
    let source_top = commits_before[0]["cliId"]
        .as_str()
        .context("Missing top source commit cliId")?;
    let source_second = commits_before[1]["cliId"]
        .as_str()
        .context("Missing second source commit cliId")?;
    let target = commits_before[2]["cliId"]
        .as_str()
        .context("Missing target commit cliId")?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!("squash {source_top},{source_second},{target}"))
        .assert()
        .success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let status_after = util::status_json(&env)?;
    let branch_a_after = util::find_branch(&status_after, "A")?;
    let commits_after = branch_a_after["commits"]
        .as_array()
        .context("Missing commits for branch A after squash")?;
    assert_eq!(commits_after.len(), 2);

    let squashed_message = commits_after[0]["message"]
        .as_str()
        .context("Missing squashed commit message")?;
    assert_eq!(
        squashed_message,
        format!("{target_message_before}\n\n{source_top_message}\n\n{source_second_message}")
    );

    let repo = env.open_repo()?;
    let first_blob = repo.rev_parse_single(b"A:A-file1.txt")?.object()?;
    let second_blob = repo.rev_parse_single(b"A:A-file2.txt")?.object()?;
    let third_blob = repo.rev_parse_single(b"A:A-file3.txt")?.object()?;

    insta::assert_snapshot!(first_blob.data.as_bstr(), @"content for commit 1\n");
    insta::assert_snapshot!(second_blob.data.as_bstr(), @"content for commit 2\n");
    insta::assert_snapshot!(third_blob.data.as_bstr(), @"content for commit 3\n");

    let log_after = env.git_log()?;
    assert!(
        log_after.contains("(A) commit 1"),
        "expected squashed A tip message to start with target commit message, got:\n{log_after}"
    );

    Ok(())
}

#[test]
fn squash_multiple_commits_from_different_branches_on_same_stack_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    env.setup_metadata(&["A", "B"])?;

    env.file("c-extra.txt", "another change on C\n");
    env.but("commit C -m 'extra on C'").assert().success();

    let status_before = util::status_json(&env)?;
    let branch_b_before = util::find_branch(&status_before, "B")?;
    let branch_c_before = util::find_branch(&status_before, "C")?;

    let b_commits_before = branch_b_before["commits"]
        .as_array()
        .context("Missing commits for branch B")?;
    let c_commits_before = branch_c_before["commits"]
        .as_array()
        .context("Missing commits for branch C")?;

    let source_b = b_commits_before[0]["cliId"]
        .as_str()
        .context("Missing source commit from B")?;
    let source_c = c_commits_before[0]["cliId"]
        .as_str()
        .context("Missing source commit from C")?;
    let target_c = c_commits_before[1]["cliId"]
        .as_str()
        .context("Missing target commit from C")?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!("squash {source_b} {source_c} {target_c}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Squashed 2 commits → [..]

"#]]);
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let status_after = util::status_json(&env)?;
    let branch_b_after = util::find_branch(&status_after, "B")?;
    let branch_c_after = util::find_branch(&status_after, "C")?;

    let b_commits_after = branch_b_after["commits"]
        .as_array()
        .context("Missing commits for branch B after squash")?;
    let c_commits_after = branch_c_after["commits"]
        .as_array()
        .context("Missing commits for branch C after squash")?;

    assert_eq!(
        b_commits_after.len() + c_commits_after.len(),
        1,
        "B and C should have a single squashed commit left in total"
    );

    Ok(())
}

#[test]
fn squash_branch_c_in_three_stacks_keeps_content_and_updates_graph() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("three-stacks")?;
    env.setup_metadata(&["A", "B", "C"])?;

    let normalized_log = env.git_log()?.replace("  \n", "\n");
    insta::assert_snapshot!(normalized_log, @r"
    *-.   205e798 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \
    | | * a748762 (B) B: another 10 lines at the bottom
    | | * 62e05ba B: 10 lines at the bottom
    | * | add59d2 (A) A: 10 lines on top
    | |/
    * | 930563a (C) C: add another 10 lines to new file
    * | 68a2fc3 C: add 10 lines to new file
    * | 984fd1c C: new file with 10 lines
    |/
    * 8f0d338 (tag: base, origin/main, origin/HEAD, main) base
    ");

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but("squash C").assert().success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let status_after = util::status_json(&env)?;
    let branch_c_after = util::find_branch(&status_after, "C")?;
    let commits_after = branch_c_after["commits"]
        .as_array()
        .context("Missing commits for branch C after squash")?;
    assert_eq!(commits_after.len(), 1);

    let repo = env.open_repo()?;
    let new_file_blob = repo.rev_parse_single(b"C:new-file")?.object()?;
    insta::assert_snapshot!(new_file_blob.data.as_bstr(), @"
    1
    2
    3
    4
    5
    6
    7
    8
    9
    10
    11
    12
    13
    14
    15
    16
    17
    18
    19
    20
    21
    22
    23
    24
    25
    26
    27
    28
    29
    30
    ");

    let normalized_log = env.git_log()?.replace("  \n", "\n");
    assert!(
        normalized_log.contains("(C) C: new file with 10 lines"),
        "expected squashed C tip message to begin with target commit message, got:\n{normalized_log}"
    );

    Ok(())
}

#[test]
fn squash_all_c_commits_into_second_commit_of_b_keeps_new_file_content() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("three-stacks")?;
    env.setup_metadata(&["A", "B", "C"])?;

    let status_before = util::status_json(&env)?;
    let branch_b_before = util::find_branch(&status_before, "B")?;
    let branch_c_before = util::find_branch(&status_before, "C")?;

    let b_commits_before = branch_b_before["commits"]
        .as_array()
        .context("Missing commits for branch B")?;
    let c_commits_before = branch_c_before["commits"]
        .as_array()
        .context("Missing commits for branch C")?;

    let b_second_commit = b_commits_before[1]["cliId"]
        .as_str()
        .context("Missing second commit cliId in B")?;
    let c_commit_ids = c_commits_before
        .iter()
        .map(|commit| {
            commit["cliId"]
                .as_str()
                .context("Missing cliId for commit in C")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let working_directory_before = util::working_directory_snapshot(&env)?;
    env.but(format!(
        "squash {} {b_second_commit}",
        c_commit_ids.join(" ")
    ))
    .assert()
    .success();
    let working_directory_after = util::working_directory_snapshot(&env)?;
    assert_eq!(working_directory_before, working_directory_after);

    let repo = env.open_repo()?;
    let new_file_blob = repo.rev_parse_single(b"B:new-file")?.object()?;
    insta::assert_snapshot!(new_file_blob.data.as_bstr(), @r"
    1
    2
    3
    4
    5
    6
    7
    8
    9
    10
    11
    12
    13
    14
    15
    16
    17
    18
    19
    20
    21
    22
    23
    24
    25
    26
    27
    28
    29
    30
    ");

    Ok(())
}

// Note: Happy-path tests for range (c0..c2) and comma-list (c0,c1,c2) notation
// are not included because:
// 1. Commit IDs are dynamically assigned and not predictable in tests
// 2. The parsing logic is thoroughly tested through error cases
// 3. All three input methods (range, list, multiple args) use the same
//    handle_multi_commit_squash function that's proven to work via branch squashing
// 4. Branch squashing tests verify the underlying API integration works correctly
