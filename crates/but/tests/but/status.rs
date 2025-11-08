use crate::utils::Sandbox;
use crate::utils::setup_metadata;

#[test]
fn worktrees() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_slow("two-worktrees")?;
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
    setup_metadata(&env, &["A", "B"])?;

    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/two-worktrees/status-with-worktrees.stdout.term.svg"
        ]);

    env.but("status --verbose")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/two-worktrees/status-with-worktrees-verbose.stdout.term.svg"
        ]);
    Ok(())
}

#[test]
fn json_shows_paths_as_strings() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    setup_metadata(&env, &["A", "B"])?;

    // Create a new file to ensure we have file assignments
    env.file("test-file.txt", "test content");

    env
        .but("--json status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "stacks": [
    [
      null,
      [
        null,
        [
          {
            "path": "test-file.txt",
            "assignments": [
              {
                "id": "[UUID]",
                "hunkHeader": {
                  "oldStart": 1,
                  "oldLines": 0,
                  "newStart": 1,
                  "newLines": 1
                },
                "path": "test-file.txt",
                "pathBytes": [
                  116,
                  101,
                  115,
                  116,
                  45,
                  102,
                  105,
                  108,
                  101,
                  46,
                  116,
                  120,
                  116
                ],
                "stackId": null,
                "lineNumsAdded": [
                  1
                ],
                "lineNumsRemoved": []
              }
            ]
          }
        ]
      ]
    ],
    [
      "[UUID]",
      [
        {
          "derivedName": "A",
          "pushStatus": "completelyUnpushed",
          "branchDetails": [
            {
              "name": "A",
              "linkedWorktreeId": null,
              "remoteTrackingBranch": null,
              "description": null,
              "prNumber": null,
              "reviewId": null,
              "tip": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
              "baseCommit": "0dc37334a458df421bf67ea806103bf5004845dd",
              "pushStatus": "completelyUnpushed",
              "lastUpdatedAt": [TIMESTAMP],
              "authors": [
                {
                  "name": "author",
                  "email": "author@example.com",
                  "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                }
              ],
              "isConflicted": false,
              "commits": [
                {
                  "id": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
                  "parentIds": [
                    "0dc37334a458df421bf67ea806103bf5004845dd"
                  ],
                  "message": "add A/n",
                  "hasConflicts": false,
                  "state": {
                    "type": "LocalOnly"
                  },
                  "createdAt": 946684800000,
                  "author": {
                    "name": "author",
                    "email": "author@example.com",
                    "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                  },
                  "gerritReviewUrl": null
                }
              ],
              "upstreamCommits": [],
              "isRemoteHead": false
            }
          ],
          "isConflicted": false
        },
        []
      ]
    ],
    [
      "[UUID]",
      [
        {
          "derivedName": "B",
          "pushStatus": "completelyUnpushed",
          "branchDetails": [
            {
              "name": "B",
              "linkedWorktreeId": null,
              "remoteTrackingBranch": null,
              "description": null,
              "prNumber": null,
              "reviewId": null,
              "tip": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
              "baseCommit": "0dc37334a458df421bf67ea806103bf5004845dd",
              "pushStatus": "completelyUnpushed",
              "lastUpdatedAt": [TIMESTAMP],
              "authors": [
                {
                  "name": "author",
                  "email": "author@example.com",
                  "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                }
              ],
              "isConflicted": false,
              "commits": [
                {
                  "id": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
                  "parentIds": [
                    "0dc37334a458df421bf67ea806103bf5004845dd"
                  ],
                  "message": "add B/n",
                  "hasConflicts": false,
                  "state": {
                    "type": "LocalOnly"
                  },
                  "createdAt": 946684800000,
                  "author": {
                    "name": "author",
                    "email": "author@example.com",
                    "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                  },
                  "gerritReviewUrl": null
                }
              ],
              "upstreamCommits": [],
              "isRemoteHead": false
            }
          ],
          "isConflicted": false
        },
        []
      ]
    ]
  ],
  "common_merge_base": {
    "target_name": "origin/main",
    "common_merge_base": "0dc3733",
    "message": "add M ",
    "commit_date": "2000-01-02 00:00:00 +0000"
  }
}

"#]]);

    Ok(())
}
