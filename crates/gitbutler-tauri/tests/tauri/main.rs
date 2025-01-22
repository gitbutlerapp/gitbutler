mod worktree {
    #[test]
    fn changes_json_sample() -> anyhow::Result<()> {
        let root = gix_testtools::scripted_fixture_read_only("status-repo.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let actual = serde_json::to_string_pretty(
            &gitbutler_tauri::worktree::worktree_changes_by_worktree_dir(root)?,
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
}
