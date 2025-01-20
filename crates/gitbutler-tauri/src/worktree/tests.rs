use super::*;

mod flags {
    use crate::worktree::Flags;
    use gix::objs::tree::EntryKind;

    #[test]
    fn calculate() {
        for ((old, new), expected) in [
            ((EntryKind::Blob, EntryKind::Blob), None),
            (
                (EntryKind::Blob, EntryKind::BlobExecutable),
                Some(Flags::ExecutableBitAdded),
            ),
            (
                (EntryKind::BlobExecutable, EntryKind::Blob),
                Some(Flags::ExecutableBitRemoved),
            ),
            (
                (EntryKind::BlobExecutable, EntryKind::Link),
                Some(Flags::TypeChangeFileToLink),
            ),
            (
                (EntryKind::Blob, EntryKind::Link),
                Some(Flags::TypeChangeFileToLink),
            ),
            (
                (EntryKind::Link, EntryKind::BlobExecutable),
                Some(Flags::TypeChangeLinkToFile),
            ),
            (
                (EntryKind::Link, EntryKind::Blob),
                Some(Flags::TypeChangeLinkToFile),
            ),
            (
                (EntryKind::Commit, EntryKind::Blob),
                Some(Flags::TypeChange),
            ),
            (
                (EntryKind::Blob, EntryKind::Commit),
                Some(Flags::TypeChange),
            ),
        ] {
            assert_eq!(Flags::calculate_inner(old, new), expected);
        }
    }
}

#[test]
fn worktree_change_json_sample() {
    let actual = serde_json::to_string_pretty(&TreeChange {
        path: "some/file".into(),
        status: Status::Modification {
            flags: Some(Flags::ExecutableBitAdded),
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
    let root = gitbutler_testsupport::gix_testtools::scripted_fixture_read_only("status_repo.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let actual = serde_json::to_string_pretty(&changes_in_worktree(root)?)?;
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
