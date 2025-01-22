//! Assure our JSON serialization doesn't break unknowingly - after all downstream may depend on it.
//!
use but_core::{ChangeState, TreeStatus};
use but_core::{ModeFlags, TreeChange, UnifiedDiff};

#[test]
fn worktree_change_json_sample() {
    let actual = serde_json::to_string_pretty(&TreeChange {
        path: "some/file".into(),
        status: TreeStatus::Modification {
            flags: Some(ModeFlags::ExecutableBitAdded),
            previous_state: ChangeState {
                id: gix::hash::Kind::Sha1.null(),
                kind: gix::object::tree::EntryKind::Blob,
            },
            state: ChangeState {
                id: gix::hash::Kind::Sha1.null(),
                kind: gix::object::tree::EntryKind::Blob,
            },
        },
    })
    .unwrap();
    insta::assert_snapshot!(actual, @r#"
    {
      "path": "some/file",
      "status": {
        "type": "Modification",
        "subject": {
          "previousState": {
            "id": "0000000000000000000000000000000000000000",
            "kind": "Blob"
          },
          "state": {
            "id": "0000000000000000000000000000000000000000",
            "kind": "Blob"
          },
          "flags": "ExecutableBitAdded"
        }
      }
    }
    "#);
}

#[test]
fn worktree_changes_example() -> anyhow::Result<()> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let repo = gix::open_opts(root, gix::open::Options::isolated())?;
    let actual = serde_json::to_string_pretty(&but_core::worktree_changes(&repo)?)?;
    insta::assert_snapshot!(actual, @r#"
    {
      "changes": [
        {
          "path": "added-to-index",
          "status": {
            "type": "Addition",
            "subject": {
              "state": {
                "id": "d95f3ad14dee633a758d2e331151e950dd13e4ed",
                "kind": "Blob"
              },
              "isUntracked": false
            }
          }
        },
        {
          "path": "executable-bit-added",
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "BlobExecutable"
              },
              "flags": "ExecutableBitAdded"
            }
          }
        },
        {
          "path": "file-to-link",
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "Link"
              },
              "flags": "TypeChangeFileToLink"
            }
          }
        },
        {
          "path": "intent-to-add",
          "status": {
            "type": "Addition",
            "subject": {
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "Blob"
              },
              "isUntracked": false
            }
          }
        },
        {
          "path": "modified-in-index",
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "9ab7cfa60ddcda5e498ef1b5330cc0ba762ebd72",
                "kind": "Blob"
              },
              "flags": null
            }
          }
        },
        {
          "path": "modified-in-worktree",
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "Blob"
              },
              "flags": null
            }
          }
        },
        {
          "path": "removed-in-index",
          "status": {
            "type": "Deletion",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              }
            }
          }
        },
        {
          "path": "removed-in-index-changed-in-worktree",
          "status": {
            "type": "Addition",
            "subject": {
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "Blob"
              },
              "isUntracked": true
            }
          }
        },
        {
          "path": "removed-in-worktree",
          "status": {
            "type": "Deletion",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              }
            }
          }
        },
        {
          "path": "untracked",
          "status": {
            "type": "Addition",
            "subject": {
              "state": {
                "id": "0000000000000000000000000000000000000000",
                "kind": "Blob"
              },
              "isUntracked": true
            }
          }
        }
      ],
      "ignoredChanges": [
        {
          "path": "conflicting",
          "status": "Conflict"
        },
        {
          "path": "removed-in-index-changed-in-worktree",
          "status": "TreeIndex"
        }
      ]
    }
    "#);
    Ok(())
}

#[test]
fn worktree_changes_unified_diffs_json_example() -> anyhow::Result<()> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let repo = gix::open_opts(&root, gix::open::Options::isolated())?;
    let diffs: Vec<UnifiedDiff> = but_core::worktree_changes(&repo)?
        .changes
        .iter()
        .map(|tree_change| tree_change.unified_diff(&repo))
        .collect::<std::result::Result<_, _>>()?;
    let actual = serde_json::to_string_pretty(&diffs)?;
    insta::assert_snapshot!(actual, @r#"
    [
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+content\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+link-target\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+content not to add to the index\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+change-in-index\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+change-in-worktree\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [
            {
              "oldStart": 1,
              "oldLines": 0,
              "newStart": 1,
              "newLines": 1,
              "diff": "@@ -1,0 +1,1 @@\n+worktree-change\n\n"
            }
          ]
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": []
        }
      }
    ]
    "#);
    Ok(())
}
