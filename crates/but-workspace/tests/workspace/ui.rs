mod changes_in_branch {
    use crate::ref_info::with_workspace_commit::utils::read_only_in_memory_scenario;
    use crate::utils::r;
    use but_graph::init::Options;
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::ui;

    #[test]
    fn multiple_inside_and_outside_of_workspace() -> anyhow::Result<()> {
        let (repo, meta) = read_only_in_memory_scenario("remote-advanced-ff")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * fb27086 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        | * 89cc2d3 (origin/A) change in A
        |/  
        * d79bba9 (A) new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");

        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/A"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);
        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/remotes/origin/A"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 1,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);
        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/gitbutler/workspace"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);

        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/remotes/origin/A"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 1,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);

        // Nothing here, it's the target.
        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/remotes/origin/main"))?, @r"
        TreeChanges {
            changes: [],
            stats: TreeStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 0,
            },
        }
        ");

        let err =
            ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/does-not-exist")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "The reference 'refs/heads/does-not-exist' did not exist",
            "passing strange ref-names still causes an error - they must exist"
        );

        let mut ref_info = ui::RefInfo::for_ui(
            but_workspace::head_info(&repo, &*meta, Default::default())?,
            &repo,
        )?
        .pruned_to_entrypoint();
        insta::assert_json_snapshot!(&ref_info, @r#"
        {
          "workspaceRef": {
            "fullNameBytes": [
              114,
              101,
              102,
              115,
              47,
              104,
              101,
              97,
              100,
              115,
              47,
              103,
              105,
              116,
              98,
              117,
              116,
              108,
              101,
              114,
              47,
              119,
              111,
              114,
              107,
              115,
              112,
              97,
              99,
              101
            ],
            "displayName": "gitbutler/workspace"
          },
          "stacks": [
            {
              "id": null,
              "base": "c166d42d4ef2e5e742d33554d03805cfb0b24d11",
              "segments": [
                {
                  "refName": {
                    "fullNameBytes": [
                      114,
                      101,
                      102,
                      115,
                      47,
                      104,
                      101,
                      97,
                      100,
                      115,
                      47,
                      65
                    ],
                    "displayName": "A"
                  },
                  "remoteTrackingRefName": {
                    "fullNameBytes": [
                      114,
                      101,
                      102,
                      115,
                      47,
                      114,
                      101,
                      109,
                      111,
                      116,
                      101,
                      115,
                      47,
                      111,
                      114,
                      105,
                      103,
                      105,
                      110,
                      47,
                      65
                    ],
                    "displayName": "A",
                    "remoteName": "origin"
                  },
                  "commits": [
                    {
                      "id": "d79bba960b112dbd25d45921c47eeda22288022b",
                      "parentIds": [
                        "c166d42d4ef2e5e742d33554d03805cfb0b24d11"
                      ],
                      "message": "new file in A\n",
                      "hasConflicts": false,
                      "state": {
                        "type": "LocalAndRemote",
                        "subject": "d79bba960b112dbd25d45921c47eeda22288022b"
                      },
                      "createdAt": 946684800000,
                      "author": {
                        "name": "author",
                        "email": "author@example.com",
                        "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                      }
                    }
                  ],
                  "commitsOnRemote": [
                    {
                      "id": "89cc2d303514654e9cab2d05b9af08b420a740c1",
                      "message": "change in A\n",
                      "createdAt": 946684800000,
                      "author": {
                        "name": "author",
                        "email": "author@example.com",
                        "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                      }
                    }
                  ],
                  "commitsOutside": null,
                  "metadata": null,
                  "isEntrypoint": false,
                  "pushStatus": "unpushedCommitsRequiringForce",
                  "base": "c166d42d4ef2e5e742d33554d03805cfb0b24d11"
                }
              ]
            }
          ],
          "target": {
            "remoteTrackingRef": {
              "fullNameBytes": [
                114,
                101,
                102,
                115,
                47,
                114,
                101,
                109,
                111,
                116,
                101,
                115,
                47,
                111,
                114,
                105,
                103,
                105,
                110,
                47,
                109,
                97,
                105,
                110
              ],
              "displayName": "main",
              "remoteName": "origin"
            },
            "commitsAhead": 0
          },
          "isManagedRef": true,
          "isManagedCommit": true,
          "isEntrypoint": true
        }
        "#);

        // Forcefully set another entrypoint to simulate the real deal.
        ref_info.is_entrypoint = false;
        ref_info
            .stacks
            .push(ref_info.stacks.first().unwrap().clone());
        ref_info
            .stacks
            .first_mut()
            .unwrap()
            .segments
            .first_mut()
            .unwrap()
            .is_entrypoint = true;
        ref_info = ref_info.pruned_to_entrypoint();
        // only one entrypoint, despite having two.
        insta::assert_json_snapshot!(&ref_info, @r#"
        {
          "workspaceRef": {
            "fullNameBytes": [
              114,
              101,
              102,
              115,
              47,
              104,
              101,
              97,
              100,
              115,
              47,
              103,
              105,
              116,
              98,
              117,
              116,
              108,
              101,
              114,
              47,
              119,
              111,
              114,
              107,
              115,
              112,
              97,
              99,
              101
            ],
            "displayName": "gitbutler/workspace"
          },
          "stacks": [
            {
              "id": null,
              "base": "c166d42d4ef2e5e742d33554d03805cfb0b24d11",
              "segments": [
                {
                  "refName": {
                    "fullNameBytes": [
                      114,
                      101,
                      102,
                      115,
                      47,
                      104,
                      101,
                      97,
                      100,
                      115,
                      47,
                      65
                    ],
                    "displayName": "A"
                  },
                  "remoteTrackingRefName": {
                    "fullNameBytes": [
                      114,
                      101,
                      102,
                      115,
                      47,
                      114,
                      101,
                      109,
                      111,
                      116,
                      101,
                      115,
                      47,
                      111,
                      114,
                      105,
                      103,
                      105,
                      110,
                      47,
                      65
                    ],
                    "displayName": "A",
                    "remoteName": "origin"
                  },
                  "commits": [
                    {
                      "id": "d79bba960b112dbd25d45921c47eeda22288022b",
                      "parentIds": [
                        "c166d42d4ef2e5e742d33554d03805cfb0b24d11"
                      ],
                      "message": "new file in A\n",
                      "hasConflicts": false,
                      "state": {
                        "type": "LocalAndRemote",
                        "subject": "d79bba960b112dbd25d45921c47eeda22288022b"
                      },
                      "createdAt": 946684800000,
                      "author": {
                        "name": "author",
                        "email": "author@example.com",
                        "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                      }
                    }
                  ],
                  "commitsOnRemote": [
                    {
                      "id": "89cc2d303514654e9cab2d05b9af08b420a740c1",
                      "message": "change in A\n",
                      "createdAt": 946684800000,
                      "author": {
                        "name": "author",
                        "email": "author@example.com",
                        "gravatarUrl": "https://www.gravatar.com/avatar/5c1e6d6e64e12aca17657581a48005d1?s=100&r=g&d=retro"
                      }
                    }
                  ],
                  "commitsOutside": null,
                  "metadata": null,
                  "isEntrypoint": true,
                  "pushStatus": "unpushedCommitsRequiringForce",
                  "base": "c166d42d4ef2e5e742d33554d03805cfb0b24d11"
                }
              ]
            }
          ],
          "target": {
            "remoteTrackingRef": {
              "fullNameBytes": [
                114,
                101,
                102,
                115,
                47,
                114,
                101,
                109,
                111,
                116,
                101,
                115,
                47,
                111,
                114,
                105,
                103,
                105,
                110,
                47,
                109,
                97,
                105,
                110
              ],
              "displayName": "main",
              "remoteName": "origin"
            },
            "commitsAhead": 0
          },
          "isManagedRef": true,
          "isManagedCommit": true,
          "isEntrypoint": false
        }
        "#);

        ref_info
            .stacks
            .first_mut()
            .unwrap()
            .segments
            .first_mut()
            .unwrap()
            .is_entrypoint = false;

        // it's Ok to have no entrypoint (even though it's not happening in practice)
        ref_info = ref_info.pruned_to_entrypoint();
        insta::assert_json_snapshot!(&ref_info, @r#"
        {
          "workspaceRef": {
            "fullNameBytes": [
              114,
              101,
              102,
              115,
              47,
              104,
              101,
              97,
              100,
              115,
              47,
              103,
              105,
              116,
              98,
              117,
              116,
              108,
              101,
              114,
              47,
              119,
              111,
              114,
              107,
              115,
              112,
              97,
              99,
              101
            ],
            "displayName": "gitbutler/workspace"
          },
          "stacks": [],
          "target": {
            "remoteTrackingRef": {
              "fullNameBytes": [
                114,
                101,
                102,
                115,
                47,
                114,
                101,
                109,
                111,
                116,
                101,
                115,
                47,
                111,
                114,
                105,
                103,
                105,
                110,
                47,
                109,
                97,
                105,
                110
              ],
              "displayName": "main",
              "remoteName": "origin"
            },
            "commitsAhead": 0
          },
          "isManagedRef": true,
          "isManagedCommit": true,
          "isEntrypoint": false
        }
        "#);
        Ok(())
    }
}
