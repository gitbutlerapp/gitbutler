use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn merge_first_branch_into_gb_local_and_verify_rebase() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches")?;

    // Run setup to create gb-local remote
    env.but("setup").assert().success();

    // Verify we're on gitbutler/workspace
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "gitbutler/workspace");

    // Create first branch
    env.but("branch new first-branch").assert().success();

    // Create first commit on first branch
    env.file("file1.txt", "content1");
    env.but("commit first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let first_branch = "first-branch";

    // Create second branch with a different commit
    env.but("branch new second-branch").assert().success();

    env.file("file2.txt", "content2");
    env.but("commit second-branch -m 'second commit on branch B'")
        .assert()
        .success();

    // Verify git log shows both branches before merge
    insta::assert_snapshot!(env.git_log()?, @r"
    *   945f3cf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * edca1cd (second-branch) second commit on branch B
    * | 549e10c (first-branch) first commit on branch A
    |/  
    * 85efbe4 (gb-local/main, gb-local/HEAD, main, gitbutler/target) M
    ");

    // Get the current main branch commit (should be the initial commit M)
    let main_before_hash = env.invoke_git("rev-parse main");

    // Merge the first branch
    env.but(format!("merge {first_branch}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"

Found 2 upstream commits on gb-local/main
   61888c9 Merge branch 'first-branch'
   549e10c first commit on branch A

Updating 2 active branches...

Rebase of second-branch successful

Branch first-branch has been integrated upstream and removed locally

Summary
────────
  second-branch - rebased
  first-branch - integrated

To undo this operation:
  Run `but undo`

"#]]);

    // Verify that main has been updated with the merge commit
    let main_after_hash = env.invoke_git("rev-parse main");

    // Main should have changed
    assert_ne!(
        main_before_hash, main_after_hash,
        "main branch should have been updated"
    );

    // Verify the merge commit has both parents
    let parents = env.invoke_git("rev-list --parents -n 1 main");
    let parent_count = parents.split_whitespace().count() - 1; // Subtract 1 for the commit itself
    assert_eq!(parent_count, 2, "Merge commit should have 2 parents");

    // Verify file1.txt exists on main now
    let file1_content = std::fs::read_to_string(env.projects_root().join("file1.txt"))?;
    assert_eq!(file1_content, "content1");

    // Verify that only the second branch remains in the workspace
    let status_after = env
        .but("status --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after_str = String::from_utf8_lossy(&status_after);
    let status_after_json: serde_json::Value = serde_json::from_str(&status_after_str)?;

    // Should only have one stack now (second-branch)
    assert_eq!(
        status_after_json["stacks"].as_array().unwrap().len(),
        1,
        "Only second-branch should remain after merge"
    );

    // Verify the second branch is rebased on top of the updated main
    let second_branch_base_hash = env.invoke_git("merge-base main second-branch");

    // The merge base should be the new main (the second branch was rebased)
    assert_eq!(
        second_branch_base_hash, main_after_hash,
        "second-branch should be rebased on top of the merged main"
    );

    // Verify git log shows the rebased structure
    insta::assert_snapshot!(env.git_log()?, @r"
    * c7f0f9d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e8d7818 (second-branch) second commit on branch B
    *   61888c9 (gb-local/main, gb-local/HEAD, main) Merge branch 'first-branch'
    |\  
    | | * 945f3cf (gb-local/gitbutler/workspace) GitButler Workspace Commit
    | |/| 
    |/| | 
    | | * edca1cd (gb-local/second-branch) second commit on branch B
    | |/  
    * / 549e10c (gb-local/first-branch) first commit on branch A
    |/  
    * 85efbe4 (gb-local/gitbutler/target, gitbutler/target) M
    ");

    Ok(())
}
