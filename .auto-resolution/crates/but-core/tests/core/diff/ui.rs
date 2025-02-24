use but_testsupport::gix_testtools;

#[test]
fn worktree_changes() -> anyhow::Result<()> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let actual = serde_json::to_string_pretty(
        &but_core::diff::ui::worktree_changes_by_worktree_dir(root.join("many-in-worktree"))?,
    )?;
    insta::assert_snapshot!(actual, @r#"
    {
      "changes": [
        {
          "path": "added-to-index",
          "pathBytes": [
            97,
            100,
            100,
            101,
            100,
            45,
            116,
            111,
            45,
            105,
            110,
            100,
            101,
            120
          ],
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
          "pathBytes": [
            101,
            120,
            101,
            99,
            117,
            116,
            97,
            98,
            108,
            101,
            45,
            98,
            105,
            116,
            45,
            97,
            100,
            100,
            101,
            100
          ],
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
          "pathBytes": [
            102,
            105,
            108,
            101,
            45,
            116,
            111,
            45,
            108,
            105,
            110,
            107
          ],
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
          "pathBytes": [
            105,
            110,
            116,
            101,
            110,
            116,
            45,
            116,
            111,
            45,
            97,
            100,
            100
          ],
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
          "pathBytes": [
            109,
            111,
            100,
            105,
            102,
            105,
            101,
            100,
            45,
            105,
            110,
            45,
            105,
            110,
            100,
            101,
            120
          ],
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
          "pathBytes": [
            109,
            111,
            100,
            105,
            102,
            105,
            101,
            100,
            45,
            105,
            110,
            45,
            119,
            111,
            114,
            107,
            116,
            114,
            101,
            101
          ],
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
          "pathBytes": [
            114,
            101,
            109,
            111,
            118,
            101,
            100,
            45,
            105,
            110,
            45,
            105,
            110,
            100,
            101,
            120
          ],
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
          "pathBytes": [
            114,
            101,
            109,
            111,
            118,
            101,
            100,
            45,
            105,
            110,
            45,
            105,
            110,
            100,
            101,
            120,
            45,
            99,
            104,
            97,
            110,
            103,
            101,
            100,
            45,
            105,
            110,
            45,
            119,
            111,
            114,
            107,
            116,
            114,
            101,
            101
          ],
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
          "path": "removed-in-worktree",
          "pathBytes": [
            114,
            101,
            109,
            111,
            118,
            101,
            100,
            45,
            105,
            110,
            45,
            119,
            111,
            114,
            107,
            116,
            114,
            101,
            101
          ],
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
          "pathBytes": [
            117,
            110,
            116,
            114,
            97,
            99,
            107,
            101,
            100
          ],
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
      "ignored_changes": [
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
fn commit_to_commit() -> anyhow::Result<()> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_dir = root.join("many-in-tree");
    let repo = &gix::open_opts(&worktree_dir, gix::open::Options::isolated())?;
    let actual = serde_json::to_string_pretty(
        &but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(
            repo,
            repo.rev_parse_single("@")?.into(),
        )?,
    )?;
    insta::assert_snapshot!(actual, @r#"
    {
      "changes": [
        {
          "path": "aa-renamed-new-name",
          "pathBytes": [
            97,
            97,
            45,
            114,
            101,
            110,
            97,
            109,
            101,
            100,
            45,
            110,
            101,
            119,
            45,
            110,
            97,
            109,
            101
          ],
          "status": {
            "type": "Rename",
            "subject": {
              "previousPath": "aa-renamed-old-name",
              "previousPathBytes": [
                97,
                97,
                45,
                114,
                101,
                110,
                97,
                109,
                101,
                100,
                45,
                111,
                108,
                100,
                45,
                110,
                97,
                109,
                101
              ],
              "previousState": {
                "id": "d95f3ad14dee633a758d2e331151e950dd13e4ed",
                "kind": "Blob"
              },
              "state": {
                "id": "d95f3ad14dee633a758d2e331151e950dd13e4ed",
                "kind": "Blob"
              },
              "flags": null
            }
          }
        },
        {
          "path": "executable-bit-added",
          "pathBytes": [
            101,
            120,
            101,
            99,
            117,
            116,
            97,
            98,
            108,
            101,
            45,
            98,
            105,
            116,
            45,
            97,
            100,
            100,
            101,
            100
          ],
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "BlobExecutable"
              },
              "flags": "ExecutableBitAdded"
            }
          }
        },
        {
          "path": "file-to-link",
          "pathBytes": [
            102,
            105,
            108,
            101,
            45,
            116,
            111,
            45,
            108,
            105,
            110,
            107
          ],
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "7ad106d48bf91c7ef87a38db2397b661a50102f5",
                "kind": "Link"
              },
              "flags": "TypeChangeFileToLink"
            }
          }
        },
        {
          "path": "modified",
          "pathBytes": [
            109,
            111,
            100,
            105,
            102,
            105,
            101,
            100
          ],
          "status": {
            "type": "Modification",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              },
              "state": {
                "id": "0835e4f9714005ed591f68d306eea0d6d2ae8fd7",
                "kind": "Blob"
              },
              "flags": null
            }
          }
        },
        {
          "path": "removed",
          "pathBytes": [
            114,
            101,
            109,
            111,
            118,
            101,
            100
          ],
          "status": {
            "type": "Deletion",
            "subject": {
              "previousState": {
                "id": "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391",
                "kind": "Blob"
              }
            }
          }
        }
      ],
      "stats": {
        "linesAdded": 2,
        "linesRemoved": 0,
        "filesChanged": 5
      }
    }
    "#);
    Ok(())
}

/// Assure our JSON serialization doesn't break unknowingly - after all downstream may depend on it.
#[test]
fn worktree_changes_unified_diffs_json_example() -> anyhow::Result<()> {
    let repo = repo("many-in-worktree")?;
    let diffs: Vec<_> = but_core::diff::worktree_changes(&repo)?
        .changes
        .iter()
        .map(|tree_change| tree_change.unified_patch(&repo, 3))
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
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
              "diff": "@@ -1,0 +1,1 @@\n+content\n"
            }
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 0,
          "linesRemoved": 0
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
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
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
              "diff": "@@ -1,0 +1,1 @@\n+content not to add to the index\n"
            }
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
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
              "diff": "@@ -1,0 +1,1 @@\n+change-in-index\n"
            }
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
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
              "diff": "@@ -1,0 +1,1 @@\n+change-in-worktree\n"
            }
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 0,
          "linesRemoved": 0
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
              "diff": "@@ -1,0 +1,1 @@\n+worktree-change\n"
            }
          ],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 1,
          "linesRemoved": 0
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 0,
          "linesRemoved": 0
        }
      },
      {
        "type": "Patch",
        "subject": {
          "hunks": [],
          "isResultOfBinaryToTextConversion": false,
          "linesAdded": 0,
          "linesRemoved": 0
        }
      }
    ]
    "#);
    Ok(())
}

pub fn repo(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    Ok(gix::open_opts(
        root.join(name),
        gix::open::Options::isolated(),
    )?)
}
