mod workspace {
    use bstr::ByteSlice;
    use but_core::ref_metadata::{
        ProjectedWorkspaceStack, StackId,
        StackKind::{Applied, AppliedAndUnapplied},
        Workspace,
        WorkspaceCommitRelation::{Merged, Outside},
        WorkspaceStack, WorkspaceStackBranch,
    };

    #[test]
    fn add_new_stack_if_not_present_journey() {
        let mut ws = Workspace::default();
        assert_eq!(ws.stacks.len(), 0);

        let a_ref = r("refs/heads/A");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, Some(100), Merged, new_stack_id),
            (0, 0)
        );
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, Some(200), Merged, new_stack_id),
            (0, 0)
        );
        assert_eq!(ws.stacks.len(), 1);

        let b_ref = r("refs/heads/B");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(b_ref, Some(0), Merged, new_stack_id),
            (0, 0)
        );
        assert_eq!(
            ws.stack_names(AppliedAndUnapplied).collect::<Vec<_>>(),
            [b_ref, a_ref]
        );

        let c_ref = r("refs/heads/C");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(c_ref, None, Merged, new_stack_id),
            (2, 0)
        );
        assert_eq!(
            ws.stack_names(AppliedAndUnapplied).collect::<Vec<_>>(),
            [b_ref, a_ref, c_ref]
        );

        assert!(ws.remove_segment(a_ref));
        assert!(ws.remove_segment(b_ref));
        assert!(!ws.remove_segment(b_ref));
        assert!(ws.remove_segment(c_ref));
        assert!(!ws.remove_segment(c_ref));

        // Everything should be removed.
        insta::assert_debug_snapshot!(ws, @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);
    }

    #[test]
    fn unapply_branch_returns_false_if_absent() {
        let mut ws = workspace(vec![
            stack(1, Merged, ["refs/heads/A"]),
            stack(2, Outside, ["refs/heads/outside"]),
        ]);

        assert!(
            !ws.unapply_branch(r("refs/heads/missing")),
            "an unknown branch is not removed from applied workspace metadata"
        );
        assert!(
            !ws.unapply_branch(r("refs/heads/outside")),
            "an outside branch is not removed from applied workspace metadata"
        );
        insta::assert_snapshot!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            "absent applied branches leave workspace metadata unchanged",
            @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
                WorkspaceStack {
                    id: 2,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/outside",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Outside,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#
        );
    }

    #[test]
    fn unapply_branch_removes_single_branch_stack() {
        let mut ws = workspace(vec![
            stack(1, Merged, ["refs/heads/A"]),
            stack(2, Merged, ["refs/heads/B"]),
        ]);

        assert!(
            ws.unapply_branch(r("refs/heads/A")),
            "single-branch applied stacks are removed entirely"
        );
        insta::assert_snapshot!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            "removing the only branch in an applied stack removes that stack metadata",
            @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#
        );
    }

    #[test]
    fn unapply_branch_marks_multi_segment_stack_outside_when_tip_is_removed() {
        let mut ws = workspace(vec![stack(
            1,
            Merged,
            ["refs/heads/A", "refs/heads/B", "refs/heads/C"],
        )]);

        assert!(
            ws.unapply_branch(r("refs/heads/A")),
            "removing the tip of a multi-segment applied stack marks it outside"
        );
        insta::assert_snapshot!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            "the status quo retains the branch metadata but moves the stack outside",
            @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Outside,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#
        );
    }

    #[test]
    fn unapply_branch_removes_middle_segment_metadata() {
        let mut ws = workspace(vec![stack(
            1,
            Merged,
            ["refs/heads/A", "refs/heads/B", "refs/heads/C"],
        )]);

        assert!(
            ws.unapply_branch(r("refs/heads/B")),
            "removing a middle segment drops that branch metadata"
        );
        insta::assert_snapshot!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            "middle segment removal keeps the stack applied and removes only that branch",
            @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#
        );
    }

    #[test]
    fn insert_new_segment_above_anchor_if_not_present_journey() {
        let mut ws = Workspace::default();
        assert_eq!(ws.stacks.len(), 0);

        let a_ref = r("refs/heads/A");
        let b_ref = r("refs/heads/B");
        assert_eq!(
            ws.insert_new_segment_above_anchor_if_not_present(b_ref, a_ref),
            None,
            "anchor doesn't exist"
        );
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, None, Merged, new_stack_id),
            (0, 0)
        );
        assert_eq!(
            ws.insert_new_segment_above_anchor_if_not_present(b_ref, a_ref),
            Some(true),
            "anchor existed and it was added"
        );
        assert_eq!(
            ws.insert_new_segment_above_anchor_if_not_present(b_ref, a_ref),
            Some(false),
            "anchor existed and it was NOT added as it already existed"
        );

        let c_ref = r("refs/heads/C");
        assert_eq!(
            ws.insert_new_segment_above_anchor_if_not_present(c_ref, a_ref),
            Some(true)
        );

        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, None, Merged, new_stack_id),
            (0, 2),
            "adding a new stack can 'fail' if the segment is already present, but not as stack tip"
        );

        insta::assert_snapshot!(but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")), @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);

        assert!(ws.remove_segment(b_ref));
        assert!(ws.remove_segment(a_ref));
        assert!(ws.remove_segment(c_ref));

        // Everything should be removed.
        insta::assert_debug_snapshot!(ws, @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);
    }

    #[test]
    fn find_owner_indexes_by_name_returns_original_stack_index_after_filtering() {
        let outside_ref = r("refs/heads/outside");
        let applied_ref = r("refs/heads/applied");
        let ws = workspace(vec![
            WorkspaceStack {
                id: StackId::from_number_for_testing(1),
                branches: vec![WorkspaceStackBranch {
                    ref_name: outside_ref.to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: StackId::from_number_for_testing(2),
                branches: vec![WorkspaceStackBranch {
                    ref_name: applied_ref.to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Merged,
            },
        ]);

        assert_eq!(
            ws.find_owner_indexes_by_name(applied_ref, Applied),
            Some((1, 0)),
            "filtered applied lookup must still return the index into the original stack list"
        );
        assert_eq!(
            ws.find_owner_indexes_by_name(outside_ref, Applied),
            None,
            "applied lookup ignores outside stacks"
        );
        assert_eq!(
            ws.find_owner_indexes_by_name(outside_ref, AppliedAndUnapplied),
            Some((0, 0)),
            "unfiltered lookup still returns outside stacks"
        );
    }

    #[test]
    fn reconcile_projected_stacks_starts_from_empty_metadata() -> anyhow::Result<()> {
        let mut ws = Workspace::default();

        ws.reconcile_projected_stacks(
            [
                projected_stack(None, ["refs/heads/A"]),
                projected_stack(None, ["refs/heads/C", "refs/heads/B"]),
            ],
            stack_id_from_name,
        )?;

        insta::assert_snapshot!(but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")), @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 1,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
                WorkspaceStack {
                    id: 2,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);
        Ok(())
    }

    #[test]
    fn reconcile_projected_stacks_is_additive_and_preserves_existing_branch_metadata()
    -> anyhow::Result<()> {
        let archived_extra_ref = r("refs/heads/old-extra");
        let mut ws = workspace(vec![WorkspaceStack {
            id: StackId::from_number_for_testing(1),
            branches: vec![
                WorkspaceStackBranch {
                    ref_name: r("refs/heads/B").to_owned(),
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: archived_extra_ref.to_owned(),
                    archived: true,
                },
            ],
            workspacecommit_relation: Outside,
        }]);

        ws.reconcile_projected_stacks(
            [projected_stack(None, ["refs/heads/C", "refs/heads/B"])],
            stack_id_from_name,
        )?;

        assert_eq!(
            branch_names(&ws.stacks[0]),
            ["refs/heads/C", "refs/heads/B", "refs/heads/old-extra"],
            "projected branches are ordered first, while unused existing branches are retained"
        );
        assert!(
            ws.stacks[0].branches[2].archived,
            "existing branch metadata is preserved"
        );
        assert_eq!(ws.stacks[0].workspacecommit_relation, Merged);
        insta::assert_debug_snapshot!(ws, @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000001,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/old-extra",
                            archived: true,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);
        Ok(())
    }

    #[test]
    fn reconcile_projected_stacks_rejects_duplicate_projected_names() {
        let mut ws = Workspace::default();
        let err = ws
            .reconcile_projected_stacks(
                [
                    projected_stack(None, ["refs/heads/A"]),
                    projected_stack(None, ["refs/heads/A"]),
                ],
                stack_id_from_name,
            )
            .expect_err("duplicate projected names violate workspace metadata constraints");

        assert_eq!(
            err.to_string(),
            "Cannot reconcile projected workspace: branch name 'refs/heads/A' occurs more than once"
        );
    }

    #[test]
    fn reconcile_projected_stacks_tolerates_duplicate_existing_metadata_names() -> anyhow::Result<()>
    {
        let matching_stack_id = StackId::from_number_for_testing(2);
        let mut ws = workspace(vec![
            WorkspaceStack {
                id: StackId::from_number_for_testing(1),
                branches: vec![WorkspaceStackBranch {
                    ref_name: r("refs/heads/A").to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: matching_stack_id,
                branches: vec![WorkspaceStackBranch {
                    ref_name: r("refs/heads/A").to_owned(),
                    archived: true,
                }],
                workspacecommit_relation: Outside,
            },
        ]);

        ws.reconcile_projected_stacks(
            [projected_stack(Some(matching_stack_id), ["refs/heads/A"])],
            stack_id_from_name,
        )?;
        insta::assert_debug_snapshot!(ws, "projected stacks prefer pulling from equally identified metadata stacks, so the Outside stacks turns Merged", @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000001,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000002,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/A",
                            archived: true,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);

        assert_eq!(
            ws.stacks.len(),
            2,
            "duplicate metadata hints in other stacks are tolerated"
        );
        assert_eq!(branch_names(&ws.stacks[0]), ["refs/heads/A"]);
        assert_eq!(branch_names(&ws.stacks[1]), ["refs/heads/A"]);
        assert!(
            ws.stacks[1].branches[0].archived,
            "the preferred stack keeps its own branch metadata"
        );
        assert_eq!(ws.stacks[1].workspacecommit_relation, Merged);
        Ok(())
    }

    #[test]
    fn reconcile_projected_stacks_moves_existing_branch_names_between_metadata_stacks()
    -> anyhow::Result<()> {
        let mut ws = workspace(vec![
            WorkspaceStack {
                id: StackId::from_number_for_testing(1),
                branches: vec![
                    WorkspaceStackBranch {
                        ref_name: r("refs/heads/B").to_owned(),
                        archived: false,
                    },
                    WorkspaceStackBranch {
                        ref_name: r("refs/heads/E").to_owned(),
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: StackId::from_number_for_testing(2),
                branches: vec![
                    WorkspaceStackBranch {
                        ref_name: r("refs/heads/C").to_owned(),
                        archived: false,
                    },
                    WorkspaceStackBranch {
                        ref_name: r("refs/heads/D").to_owned(),
                        archived: true,
                    },
                ],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: StackId::from_number_for_testing(3),
                branches: vec![WorkspaceStackBranch {
                    ref_name: r("refs/heads/unrelated").to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Merged,
            },
        ]);

        ws.reconcile_projected_stacks(
            [projected_stack(
                None,
                ["refs/heads/D", "refs/heads/C", "refs/heads/B"],
            )],
            stack_id_from_name,
        )?;

        insta::assert_debug_snapshot!(ws, "projected stack grouping moves existing branch metadata between stacks, while unrelated branch metadata remains", @r#"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000001,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/E",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000002,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/D",
                            archived: true,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/C",
                            archived: false,
                        },
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/B",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
                WorkspaceStack {
                    id: 00000000-0000-0000-0000-000000000003,
                    branches: [
                        WorkspaceStackBranch {
                            ref_name: "refs/heads/unrelated",
                            archived: false,
                        },
                    ],
                    workspacecommit_relation: Merged,
                },
            ],
            target_ref: None,
            target_commit_id: None,
            push_remote: None,
        }
        "#);
        Ok(())
    }

    #[test]
    fn reconcile_projected_stacks_removes_stacks_emptied_by_moves() -> anyhow::Result<()> {
        let mut ws = workspace(vec![
            WorkspaceStack {
                id: StackId::from_number_for_testing(1),
                branches: vec![WorkspaceStackBranch {
                    ref_name: r("refs/heads/B").to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: StackId::from_number_for_testing(2),
                branches: vec![WorkspaceStackBranch {
                    ref_name: r("refs/heads/C").to_owned(),
                    archived: false,
                }],
                workspacecommit_relation: Merged,
            },
        ]);

        ws.reconcile_projected_stacks(
            [projected_stack(None, ["refs/heads/C", "refs/heads/B"])],
            stack_id_from_name,
        )?;

        assert_eq!(
            ws.stacks.len(),
            1,
            "the stack that only contained moved branch B should be removed"
        );
        assert_eq!(
            branch_names(&ws.stacks[0]),
            ["refs/heads/C", "refs/heads/B"]
        );
        Ok(())
    }

    fn workspace(stacks: Vec<WorkspaceStack>) -> Workspace {
        let mut ws = Workspace::default();
        ws.stacks = stacks;
        ws
    }

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known ref")
    }
    fn new_stack_id(_: &gix::refs::FullNameRef) -> StackId {
        StackId::generate()
    }
    fn stack_id_from_name(name: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(name.shorten().chars().map(|c| c as u128).sum())
    }
    fn projected_stack<const N: usize>(
        id: Option<StackId>,
        branches: [&str; N],
    ) -> ProjectedWorkspaceStack {
        ProjectedWorkspaceStack {
            id,
            branches: branches
                .into_iter()
                .map(|name| r(name).to_owned())
                .collect(),
        }
    }
    fn stack<const N: usize>(
        id: u128,
        workspacecommit_relation: but_core::ref_metadata::WorkspaceCommitRelation,
        branches: [&str; N],
    ) -> WorkspaceStack {
        WorkspaceStack {
            id: StackId::from_number_for_testing(id),
            branches: branches
                .into_iter()
                .map(|name| WorkspaceStackBranch {
                    ref_name: r(name).to_owned(),
                    archived: false,
                })
                .collect(),
            workspacecommit_relation,
        }
    }
    fn branch_names(stack: &WorkspaceStack) -> Vec<&str> {
        stack
            .branches
            .iter()
            .map(|branch| branch.ref_name.as_bstr().to_str().expect("utf8"))
            .collect()
    }
}

mod project_meta {
    use but_core::ref_metadata::ProjectMeta;
    use but_testsupport::read_only_in_memory_scenario;

    #[test]
    fn malformed_target_ref_and_commit_id_read_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\
             \ttargetRef = origin/master\n\
             \ttargetCommitId = not-a-commit-id\n\
             \tpushRemote = upstream\n",
        )?;

        let actual = ProjectMeta::try_from_config(&config)?;
        assert_eq!(
            actual.target_ref, None,
            "a target ref that isn't a full ref name is ignored instead of failing the whole read"
        );
        assert_eq!(
            actual.target_commit_id, None,
            "a target commit id that isn't a hexadecimal object id is ignored as well"
        );
        assert_eq!(
            actual.push_remote.as_deref(),
            Some("upstream"),
            "well-formed values are still read despite malformed siblings"
        );
        Ok(())
    }

    #[test]
    fn non_remote_target_ref_reads_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\
             \ttargetRef = refs/heads/main\n",
        )?;

        let actual = ProjectMeta::try_from_config(&config)?;
        assert_eq!(
            actual.target_ref, None,
            "a target ref that isn't a remote tracking branch would wrongly be seeded as remote \
             target tip, so it's ignored"
        );
        Ok(())
    }

    #[test]
    fn push_remote_name_falls_back_to_textual_remote_name() -> anyhow::Result<()> {
        let repo = read_only_in_memory_scenario("multiple-remotes-with-tracking-branches")?;

        let meta = ProjectMeta {
            target_ref: Some(gix::refs::FullName::try_from(
                "refs/remotes/gone/release/1.x".to_owned(),
            )?),
            target_commit_id: None,
            push_remote: None,
        };
        assert_eq!(
            meta.push_remote_name(&repo)?,
            "gone",
            "with no matching configured remote and a slash in the branch name, \
             the first path component after refs/remotes/ is used, like legacy metadata stored"
        );

        let meta = ProjectMeta {
            target_ref: Some(gix::refs::FullName::try_from(
                "refs/remotes/nested/remote/feature/a".to_owned(),
            )?),
            target_commit_id: None,
            push_remote: None,
        };
        assert_eq!(
            meta.push_remote_name(&repo)?,
            "nested/remote",
            "configured remotes remain the primary path so remote names containing '/' still work"
        );
        Ok(())
    }

    #[test]
    fn null_target_commit_id_reads_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\
             \ttargetRef = refs/remotes/origin/main\n\
             \ttargetCommitId = 0000000000000000000000000000000000000000\n",
        )?;

        let actual = ProjectMeta::try_from_config(&config)?;
        assert_eq!(
            actual.target_ref.map(|name| name.to_string()),
            Some("refs/remotes/origin/main".to_string())
        );
        assert_eq!(
            actual.target_commit_id, None,
            "the null id is a placeholder for an unknown commit and must read as absent"
        );
        Ok(())
    }
}
