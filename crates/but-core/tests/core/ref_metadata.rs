mod workspace {
    use bstr::ByteSlice;
    use but_core::ref_metadata::{
        ProjectedWorkspaceStack, StackId,
        StackKind::{Applied, AppliedAndUnapplied},
        Workspace,
        WorkspaceCommitRelation::{Merged, Outside},
        WorkspaceStack, WorkspaceStackBranch,
    };
    use snapbox::prelude::*;

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
        snapbox::assert_data_eq!(
            ws.to_debug(),
            snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: None, updated_at: None },
    stacks: [],
}

"#]]
        );
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
        // absent applied branches leave workspace metadata unchanged
        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
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
        // removing the only branch in an applied stack removes that stack metadata
        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
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
        // the status quo retains the branch metadata but moves the stack outside
        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
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
        // middle segment removal keeps the stack applied and removes only that branch
        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
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

        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
        );

        assert!(ws.remove_segment(b_ref));
        assert!(ws.remove_segment(a_ref));
        assert!(ws.remove_segment(c_ref));

        // Everything should be removed.
        snapbox::assert_data_eq!(
            ws.to_debug(),
            snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: None, updated_at: None },
    stacks: [],
}

"#]]
        );
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

        snapbox::assert_data_eq!(
            but_testsupport::sanitize_uuids_and_timestamps(format!("{ws:#?}")),
            snapbox::str![[r#"
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
}
"#]]
        );
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
        snapbox::assert_data_eq!(
            ws.to_debug(),
            snapbox::str![[r#"
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
}

"#]]
        );
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
        // projected stacks prefer pulling from equally identified metadata stacks, so the Outside stacks turns Merged
        snapbox::assert_data_eq!(
            ws.to_debug(),
            snapbox::str![[r#"
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
}

"#]]
        );

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

        // projected stack grouping moves existing branch metadata between stacks, while unrelated branch metadata remains
        snapbox::assert_data_eq!(
            ws.to_debug(),
            snapbox::str![[r#"
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
}

"#]]
        );
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
        Workspace {
            stacks,
            ..Default::default()
        }
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
    use but_core::ref_metadata::{
        LocalRefName, ProjectMeta, UnresolvedLegacyTargetReason, UnresolvedLegacyTargetRefError,
    };
    use but_testsupport::read_only_in_memory_scenario;

    fn repo() -> anyhow::Result<gix::Repository> {
        read_only_in_memory_scenario("project-target-metadata")
    }

    fn local_target(name: &str) -> anyhow::Result<ProjectMeta> {
        Ok(ProjectMeta {
            target_ref: Some(LocalRefName::new(name.try_into()?)?),
            ..ProjectMeta::default()
        })
    }

    fn legacy_config() -> anyhow::Result<gix::config::File> {
        Ok(gix::config::File::try_from(
            "[gitbutler \"project\"]\n\ttargetRef = refs/remotes/origin/trunk\n",
        )?)
    }

    #[test]
    fn migration_repair_preserves_an_unset_cutoff() -> anyhow::Result<()> {
        let meta = local_target("refs/heads/main")?;
        let repaired =
            but_core::ref_metadata::repair_target_metadata_for_migration(&meta, &repo()?);
        assert_eq!(
            repaired, meta,
            "repair must not pin an unset cutoff to local history"
        );
        Ok(())
    }

    #[test]
    fn persisting_metadata_preserves_an_unset_cutoff() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("project-target-metadata");
        let meta = local_target("refs/heads/main")?;
        meta.persist(&repo)?;
        assert_eq!(
            ProjectMeta::resolve(&repo)?,
            meta,
            "persistence must retain dynamic cutoff selection"
        );
        Ok(())
    }

    #[test]
    fn porting_metadata_preserves_an_unset_cutoff() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("project-target-metadata");
        let meta = local_target("refs/heads/main")?;
        ProjectMeta::port_if_needed(&repo, || Ok(meta.clone()))?;
        assert_eq!(
            ProjectMeta::resolve(&repo)?,
            meta,
            "migration must not invent a stored cutoff"
        );
        Ok(())
    }

    #[test]
    fn malformed_target_ref_and_commit_id_read_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\
             \ttargetRef = origin/master\n\
             \ttargetCommitId = not-a-commit-id\n\
             \tpushRemote = upstream\n",
        )?;
        let actual = ProjectMeta::try_from_config(&config, &repo()?)?;
        assert_eq!(actual.target_ref, None, "malformed ref names are ignored");
        assert_eq!(
            actual.target_commit_id, None,
            "malformed commit IDs are ignored"
        );
        assert_eq!(
            actual.push_remote.as_deref(),
            Some("upstream"),
            "valid siblings survive"
        );
        Ok(())
    }

    #[test]
    fn local_target_ref_is_read() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\ttargetRef = refs/heads/main\n",
        )?;
        let actual = ProjectMeta::try_from_config(&config, &repo()?)?;
        assert_eq!(
            actual,
            local_target("refs/heads/main")?,
            "local targets are accepted"
        );
        Ok(())
    }

    #[test]
    fn tag_target_ref_reads_as_none() -> anyhow::Result<()> {
        let config =
            gix::config::File::try_from("[gitbutler \"project\"]\n\ttargetRef = refs/tags/main\n")?;
        let actual = ProjectMeta::try_from_config(&config, &repo()?)?;
        assert_eq!(actual.target_ref, None, "tags are not integration targets");
        Ok(())
    }

    #[test]
    fn workspace_target_ref_reads_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\ttargetRef = refs/heads/gitbutler/workspace\n",
        )?;
        let actual = ProjectMeta::try_from_config(&config, &repo()?)?;
        assert_eq!(
            actual.target_ref, None,
            "a stored workspace target is ignored so the project loads and asks for a target again"
        );
        Ok(())
    }

    #[test]
    fn null_target_commit_id_reads_as_none() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\
             \ttargetRef = refs/heads/main\n\
             \ttargetCommitId = 0000000000000000000000000000000000000000\n",
        )?;
        let actual = ProjectMeta::try_from_config(&config, &repo()?)?;
        assert_eq!(
            actual,
            local_target("refs/heads/main")?,
            "the null commit ID means absent"
        );
        Ok(())
    }

    #[test]
    fn tracking_lookup_returns_unique_local_branch() -> anyhow::Result<()> {
        let actual = but_core::ref_metadata::find_local_tracking_branch(
            "refs/remotes/origin/trunk".try_into()?,
            &repo()?,
        )?;
        assert_eq!(
            actual,
            but_core::ref_metadata::LocalTrackingBranchMatch::Unique(LocalRefName::new(
                "refs/heads/main".try_into()?
            )?),
            "the lookup follows configured upstreams, not matching branch names"
        );
        Ok(())
    }

    #[test]
    fn tracking_lookup_returns_no_match() -> anyhow::Result<()> {
        let actual = but_core::ref_metadata::find_local_tracking_branch(
            "refs/remotes/origin/missing".try_into()?,
            &repo()?,
        )?;
        assert_eq!(
            actual,
            but_core::ref_metadata::LocalTrackingBranchMatch::None,
            "a missing counterpart is a match result, not a repository error"
        );
        Ok(())
    }

    #[test]
    fn tracking_lookup_reports_ambiguous_branches() -> anyhow::Result<()> {
        let mut repo = repo()?;
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.other.remote", "origin")?;
            config.set_raw_value("branch.other.merge", "refs/heads/trunk")?;
        }
        let actual = but_core::ref_metadata::find_local_tracking_branch(
            "refs/remotes/origin/trunk".try_into()?,
            &repo,
        )?;
        assert_eq!(
            actual,
            but_core::ref_metadata::LocalTrackingBranchMatch::Ambiguous {
                first: LocalRefName::new("refs/heads/main".try_into()?)?,
                second: LocalRefName::new("refs/heads/other".try_into()?)?,
            },
            "ambiguity preserves both names for explicit-resolution diagnostics"
        );
        Ok(())
    }

    #[test]
    fn tracking_lookup_prefers_branch_named_as_on_remote() -> anyhow::Result<()> {
        let mut repo = repo()?;
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.main.merge", "refs/heads/other")?;
            config.set_raw_value("branch.other.remote", "origin")?;
            config.set_raw_value("branch.other.merge", "refs/heads/other")?;
        }
        let actual = but_core::ref_metadata::find_local_tracking_branch(
            "refs/remotes/origin/other".try_into()?,
            &repo,
        )?;
        assert_eq!(
            actual,
            but_core::ref_metadata::LocalTrackingBranchMatch::Unique(LocalRefName::new(
                "refs/heads/other".try_into()?
            )?),
            "of the branches tracking origin/other, local other is its counterpart, \
             even though main is found first"
        );
        Ok(())
    }

    #[test]
    fn resolve_local_target_accepts_local_name_without_lookup() -> anyhow::Result<()> {
        let repo = repo()?;
        let name: &gix::refs::FullNameRef = "refs/heads/not-created".try_into()?;
        let actual = but_core::ref_metadata::resolve_local_target(name, &repo)?;
        assert_eq!(
            actual.as_local_full_name_ref(),
            name,
            "local names are validated without requiring an existing branch or upstream"
        );
        Ok(())
    }

    #[test]
    fn resolve_local_target_rejects_tag() -> anyhow::Result<()> {
        let repo = repo()?;
        let err = but_core::ref_metadata::resolve_local_target("refs/tags/main".try_into()?, &repo)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("must be a local or remote-tracking branch"),
            "non-branch refs must fail before tracking lookup: {err}"
        );
        Ok(())
    }

    #[test]
    fn local_ref_name_rejects_remote_branch() -> anyhow::Result<()> {
        assert!(
            LocalRefName::new("refs/remotes/origin/main".try_into()?).is_err(),
            "the type must reject remote-tracking refs"
        );
        Ok(())
    }

    #[test]
    fn local_ref_name_rejects_tag() -> anyhow::Result<()> {
        assert!(
            LocalRefName::new("refs/tags/main".try_into()?).is_err(),
            "the type must reject tags"
        );
        Ok(())
    }

    #[test]
    fn legacy_remote_resolves_differently_named_local_branch() -> anyhow::Result<()> {
        let actual = ProjectMeta::try_from_config(&legacy_config()?, &repo()?)?;
        assert_eq!(
            actual,
            local_target("refs/heads/main")?,
            "upstream configuration determines the local target"
        );
        Ok(())
    }

    #[test]
    fn legacy_remote_without_local_match_errors() -> anyhow::Result<()> {
        let config = gix::config::File::try_from(
            "[gitbutler \"project\"]\n\ttargetRef = refs/remotes/origin/missing\n",
        )?;
        let err = ProjectMeta::try_from_config(&config, &repo()?).unwrap_err();
        assert_eq!(
            err.downcast_ref::<UnresolvedLegacyTargetRefError>()
                .map(|err| err.reason),
            Some(UnresolvedLegacyTargetReason::NoTrackingBranch),
            "missing local counterparts require an explicit choice: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn legacy_remote_with_multiple_local_matches_errors() -> anyhow::Result<()> {
        let mut repo = repo()?;
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.other.remote", "origin")?;
            config.set_raw_value("branch.other.merge", "refs/heads/trunk")?;
        }
        let err = ProjectMeta::try_from_config(&legacy_config()?, &repo).unwrap_err();
        assert_eq!(
            err.downcast_ref::<UnresolvedLegacyTargetRefError>()
                .map(|err| err.reason),
            Some(UnresolvedLegacyTargetReason::MultipleTrackingBranches),
            "ambiguous local counterparts must not be guessed: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn legacy_conversion_preserves_commit_ahead_of_local_target() -> anyhow::Result<()> {
        let repo = repo()?;
        let upstream = repo
            .find_reference("refs/remotes/origin/trunk")?
            .peel_to_commit()?
            .id;
        let local = repo.find_reference("refs/heads/main")?.peel_to_commit()?.id;
        assert_ne!(
            upstream, local,
            "the fixture's local target trails upstream"
        );
        let config = gix::config::File::try_from(format!(
            "[gitbutler \"project\"]\n\ttargetRef = refs/remotes/origin/trunk\n\ttargetCommitId = {upstream}\n"
        ).as_str())?;
        let actual = ProjectMeta::try_from_config(&config, &repo)?;
        assert_eq!(
            actual.target_ref,
            local_target("refs/heads/main")?.target_ref,
            "the ref is converted"
        );
        assert_eq!(
            actual.target_commit_id,
            Some(upstream),
            "migration preserves the stable cutoff"
        );
        Ok(())
    }

    #[test]
    fn remote_target_uses_configured_upstream_name() -> anyhow::Result<()> {
        let actual = local_target("refs/heads/main")?.remote_target_ref(&repo()?)?;
        assert_eq!(
            actual,
            Some("refs/remotes/origin/trunk".try_into()?),
            "upstream names need not match local names"
        );
        Ok(())
    }

    #[test]
    fn remote_target_does_not_require_existing_ref() -> anyhow::Result<()> {
        let mut repo = repo()?;
        repo.config_snapshot_mut()
            .set_raw_value("branch.main.merge", "refs/heads/not-fetched")?;
        let actual = local_target("refs/heads/main")?.remote_target_ref(&repo)?;
        assert_eq!(
            actual,
            Some("refs/remotes/origin/not-fetched".try_into()?),
            "resolution returns a name, not an existence check"
        );
        Ok(())
    }

    #[test]
    fn local_target_without_upstream_has_no_remote_target() -> anyhow::Result<()> {
        let meta = local_target("refs/heads/other")?;
        let repo = repo()?;
        assert_eq!(
            meta.remote_target_ref(&repo)?,
            None,
            "local-only targets are supported"
        );
        assert!(
            meta.remote_target_ref_or_err(&repo).is_err(),
            "remote-required operations reject local-only targets"
        );
        Ok(())
    }

    #[test]
    fn missing_target_has_no_remote_target() -> anyhow::Result<()> {
        let meta = ProjectMeta::default();
        let repo = repo()?;
        assert_eq!(
            meta.remote_target_ref(&repo)?,
            None,
            "no target means no upstream"
        );
        let err = meta.remote_target_ref_or_err(&repo).unwrap_err();
        assert!(
            matches!(
                err.downcast_ref::<but_error::Code>(),
                Some(but_error::Code::DefaultTargetNotFound)
            ),
            "the required accessor preserves the missing-target error code"
        );
        Ok(())
    }

    #[test]
    fn push_remote_name_falls_back_to_target_fetch_remote() -> anyhow::Result<()> {
        assert_eq!(
            local_target("refs/heads/main")?.push_remote_name(&repo()?)?,
            "origin",
            "feature publishing falls back to the target's fetch remote"
        );
        Ok(())
    }

    #[test]
    fn explicit_push_remote_overrides_target_remote() -> anyhow::Result<()> {
        let repo = repo()?;
        let mut meta = local_target("refs/heads/main")?;
        meta.push_remote = Some("fork".into());
        assert_eq!(
            meta.push_remote_name(&repo)?,
            "fork",
            "explicit publishing remote wins"
        );
        assert_eq!(
            meta.push_remote_url(&repo)?,
            "https://example.com/fork.git",
            "publishing uses the override URL"
        );
        assert_eq!(
            meta.target_remote_url(&repo)?,
            "https://example.com/upstream.git",
            "target fetch URL ignores the publishing override"
        );
        Ok(())
    }

    #[test]
    fn target_remote_url_uses_fetch_url() -> anyhow::Result<()> {
        let mut repo = repo()?;
        repo.config_snapshot_mut()
            .set_raw_value("remote.origin.pushurl", "https://example.com/push.git")?;
        assert_eq!(
            local_target("refs/heads/main")?.target_remote_url(&repo)?,
            "https://example.com/upstream.git",
            "target remote URL is for fetching, not pushing"
        );
        Ok(())
    }
}
