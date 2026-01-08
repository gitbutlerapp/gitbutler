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
        .env("CLICOLOR_FORCE", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/two-worktrees/status-with-worktrees.stdout.term.svg"
        ]);

    env.but("status --verbose")
        .env("CLICOLOR_FORCE", "1")
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
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "test-file.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
              "cliId": "94",
              "commitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
              "createdAt": "2000-01-01T00:00:00+00:00",
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
          "reviewId": null,
          "ci": null
        }
      ]
    },
    {
      "cliId": "h0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "h0",
          "name": "B",
          "commits": [
            {
              "cliId": "d3",
              "commitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
              "createdAt": "2000-01-01T00:00:00+00:00",
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
          "reviewId": null,
          "ci": null
        }
      ]
    }
  ],
  "mergeBase": {
    "cliId": "0d",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "2000-01-02T00:00:00+00:00",
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
      "createdAt": "2000-01-02T00:00:00+00:00",
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

// TODO This test demonstrates how IDs are assigned to uncommitted and committed
// files that have multiple hunks. This test can be removed when we have CLI
// IDs for hunks, a command (e.g. `rub`) is taught to use them, and that command
// is tested.
#[test]
fn uncommitted_and_committed_file_cli_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"])?;

    env.file("a.txt", format!("first\n{}last\n", "line\n".repeat(100)));
    env.file("b.txt", "only\n");
    env.but("commit A -m create-a-and-b").assert().success();
    env.file("a.txt", format!("firsta\n{}lasta\n", "line\n".repeat(100)));
    env.file("b.txt", "onlya\n");
    env.but("commit A -m edit-a-and-b").assert().success();
    env.file("a.txt", format!("firstb\n{}lastb\n", "line\n".repeat(100)));
    env.file("b.txt", "onlyb\n");

    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "a.txt",
      "changeType": "modified"
    },
    {
      "cliId": "j0",
      "filePath": "b.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
...
              "message": "edit-a-and-b",
...
              "changes": [
                {
                  "cliId": "q0",
                  "filePath": "a.txt",
                  "changeType": "modified"
                },
                {
                  "cliId": "r0",
                  "filePath": "b.txt",
                  "changeType": "modified"
                }
              ]
...
              "message": "create-a-and-b",
...
              "changes": [
                {
                  "cliId": "n0",
                  "filePath": "a.txt",
                  "changeType": "added"
                },
                {
                  "cliId": "o0",
                  "filePath": "b.txt",
                  "changeType": "added"
                }
              ]
...

"#]]);

    Ok(())
}

#[test]
fn long_cli_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"])?;

    // For "add A13" and "add A3", the IDs have 3 characters. The others have 2.
    env.but("status")
        .env("CLICOLOR_FORCE", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/long-cli-ids.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn long_cli_ids_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"])?;

    // Assert a handful of commits to show that the commit CLI IDs become longer
    // if a short ID would be ambiguous, but remain at 2 characters otherwise.
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
...
          "commits": [
            {
              "cliId": "5c8",
              "commitId": "5c88a8ec10067ef547f14b467776d3584cd683ea",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A13/n",
...
            {
              "cliId": "a1",
              "commitId": "a18ea48cd317c7c8fc9317b6f2427be4cdb2585d",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A12/n",
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
              "cliId": "5c7",
              "commitId": "5c7c6d7f3854bb61978b410b1ae8146be9948b26",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A3/n",
...

"#]]);

    Ok(())
}
