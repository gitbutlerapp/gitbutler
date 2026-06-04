use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn integrate_first_branch_into_origin_and_verify_rebase() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head")?;

    let remote = env.projects_root().with_extension("origin.git");
    env.invoke_git(&format!("init --bare {}", remote.display()));
    env.invoke_git(&format!(
        "--git-dir={} symbolic-ref HEAD refs/heads/main",
        remote.display()
    ));
    env.invoke_git(&format!("remote set-url origin {}", remote.display()));
    env.invoke_git("push origin main:main");
    env.invoke_git("fetch origin");
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

    let main_before_hash = env.invoke_git("rev-parse main");
    let origin_main_before_hash = env.invoke_git("rev-parse origin/main");

    // Integrate the first branch
    let output = env
        .but(format!("integrate {first_branch} --yes"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("This will directly push to origin/main. Are you sure?"),
        "integrate should warn before directly pushing the target branch"
    );
    assert!(
        stdout.contains("Branch first-branch has been integrated upstream and removed locally"),
        "integrate should use pull's active-branch cleanup after pushing"
    );
    assert!(
        stdout.contains("Integration complete!"),
        "integrate should report completion after cleanup"
    );

    // Verify that only the remote target was updated directly.
    let main_after_hash = env.invoke_git("rev-parse main");
    let origin_main_after_hash = env.invoke_git("rev-parse origin/main");

    assert_eq!(
        main_before_hash, main_after_hash,
        "integrate should directly update the remote target, not the local main ref"
    );
    assert_ne!(
        origin_main_before_hash, origin_main_after_hash,
        "origin/main should advance after integrate pushes the merge commit"
    );

    // Verify the merge commit has both parents
    let parents = env.invoke_git("rev-list --parents -n 1 origin/main");
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
        "Only second-branch should remain after integrate"
    );

    // Verify the second branch is rebased on top of the updated main
    let second_branch_base_hash = env.invoke_git("merge-base origin/main second-branch");

    // The merge base should be the new target (the second branch was rebased)
    assert_eq!(
        second_branch_base_hash, origin_main_after_hash,
        "second-branch should be rebased on top of the merged target"
    );

    Ok(())
}

#[test]
fn integrate_updates_local_target_for_self_remote() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches")?;

    env.but("setup").assert().success();
    env.but("branch new first-branch").assert().success();

    env.file("file1.txt", "content1");
    env.but("commit first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let main_before = env.invoke_git("rev-parse main");
    let gb_local_main_before = env.invoke_git("rev-parse gb-local/main");
    assert_eq!(main_before, gb_local_main_before);

    let output = env
        .but("integrate first-branch --yes")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("Integration complete!"),
        "integrate should report completion after updating gb-local targets locally"
    );
    assert!(
        stdout.contains("Branch first-branch has been integrated upstream and removed locally"),
        "integrate should use pull's active-branch cleanup after updating the target"
    );

    let main_after = env.invoke_git("rev-parse main");
    let gb_local_main_after = env.invoke_git("rev-parse gb-local/main");
    assert_ne!(
        main_before, main_after,
        "main should advance after integrate"
    );
    assert_eq!(
        main_after, gb_local_main_after,
        "gb-local/main should track the updated local target"
    );

    let parents = env.invoke_git("rev-list --parents -n 1 main");
    let parent_count = parents.split_whitespace().count() - 1;
    assert_eq!(parent_count, 2, "Merge commit should have 2 parents");

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
    assert_eq!(
        status_after_json["stacks"].as_array().unwrap().len(),
        0,
        "the branch should be removed from the workspace after local target integration"
    );

    Ok(())
}

#[test]
fn pull_integrates_branch_when_gb_local_target_points_at_branch_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches")?;

    env.but("setup").assert().success();
    env.but("branch new first-branch").assert().success();

    env.file("file1.txt", "content1");
    env.but("commit first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let branch_head = env.invoke_git("rev-parse first-branch");
    let main_before = env.invoke_git("rev-parse main");
    assert_ne!(
        branch_head, main_before,
        "test setup must leave main behind the virtual branch"
    );

    env.invoke_git("update-ref refs/heads/main first-branch");

    env.but("pull").assert().success().stdout_eq(str![[r#"

Found 1 upstream commits on gb-local/main
   [..] first commit on branch A

Updating 1 active branches...


Branch first-branch has been integrated upstream and removed locally

Summary
────────
  first-branch - integrated

To undo this operation:
  Run `but undo`

"#]]);

    let main_after = env.invoke_git("rev-parse main");
    assert_eq!(
        main_after, branch_head,
        "main should still point at the manually integrated branch head"
    );

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
    assert_eq!(
        status_after_json["stacks"].as_array().unwrap().len(),
        0,
        "the branch should be removed from the workspace after base integration"
    );
    assert_eq!(
        status_after_json["upstreamState"]["behind"].as_u64(),
        Some(0),
        "the stored base should be advanced to the updated gb-local target"
    );

    Ok(())
}
