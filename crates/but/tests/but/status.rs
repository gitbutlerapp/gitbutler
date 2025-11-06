use crate::utils::Sandbox;
use crate::utils::setup_metadata;

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
    {
      "stack_id": null,
      "details": null,
      "assignments": [
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
          ],
          "cli_id": "xe"
        }
      ],
      "cli_id": "00"
    },
    {
      "stack_id": "[UUID]",
      "details": {
        "derived_name": "A",
        "push_status": "completelyUnpushed",
        "branch_details": [
          {
            "name": "A",
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
            "isRemoteHead": false,
            "cli_id": "l3"
          }
        ],
        "is_conflicted": false,
        "cli_id": "l3"
      },
      "assignments": [],
      "cli_id": "l3"
    },
    {
      "stack_id": "[UUID]",
      "details": {
        "derived_name": "B",
        "push_status": "completelyUnpushed",
        "branch_details": [
          {
            "name": "B",
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
            "isRemoteHead": false,
            "cli_id": "m3"
          }
        ],
        "is_conflicted": false,
        "cli_id": "m3"
      },
      "assignments": [],
      "cli_id": "m3"
    }
  ],
  "common_merge_base": {
    "target_name": "origin/main",
    "common_merge_base": "0dc3733",
    "message": "add M "
  }
}

"#]]);

    Ok(())
}
