use crate::utils::Sandbox;

#[test]
fn worktrees() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   063d8c1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 3e01e28 (B) B
    * | 4c4624e (A) A
    |/  
    | * 8dc508f (origin/main, origin/HEAD, main) M-advanced
    |/  
    | * 197ddce (origin/A) A-remote
    |/  
    * 081bae9 M-base
    * 3183e43 M1
    ");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"])?;

    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/two-worktrees/status-with-worktrees.stdout.term.svg"
        ]);

    env.but("status --verbose")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/two-worktrees/status-with-worktrees-verbose.stdout.term.svg"
        ]);
    Ok(())
}

#[test]
fn unborn() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("unborn")?;
    insta::assert_snapshot!(env.git_log()?, @r"");

    // TODO: make this work
    env.but("status --verbose")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No push remote set

"#]])
        .stdout_eq(snapbox::str![]);
    Ok(())
}

#[test]
fn first_commit_no_workspace() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("first-commit")?;
    insta::assert_snapshot!(env.git_log()?, @"* 85efbe4 (HEAD -> main) M");

    // TODO: make this work
    env.but("status --verbose")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No push remote set

"#]])
        .stdout_eq(snapbox::str![]);
    Ok(())
}

#[test]
fn json_shows_paths_as_strings() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"])?;

    // Create a new file to ensure we have file assignments
    env.file("test-file.txt", "test content");

    env.but("--json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "g0",
      "filePath": "test-file.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "l3",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "l3",
          "name": "A",
          "commits": [
            {
              "cliId": "94",
              "commitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A/n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            }
          ],
          "upstreamCommits": [],
          "branchStatus": "completelyUnpushed",
          "reviewId": null
        }
      ]
    },
    {
      "cliId": "m3",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "m3",
          "name": "B",
          "commits": [
            {
              "cliId": "d3",
              "commitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add B/n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            }
          ],
          "upstreamCommits": [],
          "branchStatus": "completelyUnpushed",
          "reviewId": null
        }
      ]
    }
  ],
  "mergeBase": {
    "cliId": "0d",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "[RFC_TIMESTAMP]",
    "message": "add M ",
    "authorName": "author",
    "authorEmail": "author@example.com",
    "conflicted": null,
    "reviewId": null,
    "changes": null
  },
  "upstreamState": {
    "behind": 0,
    "latestCommit": {
      "cliId": "0d",
      "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
      "createdAt": "[RFC_TIMESTAMP]",
      "message": "add M ",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}

"#]]);

    Ok(())
}
