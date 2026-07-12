mod workspace {
    use but_core::ref_metadata::{
        StackId,
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
                    parents: None,
                }],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: StackId::from_number_for_testing(2),
                branches: vec![WorkspaceStackBranch {
                    ref_name: applied_ref.to_owned(),
                    archived: false,
                    parents: None,
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

    mod dag_declaration {
        use super::{r, stack};
        use but_core::ref_metadata::WorkspaceCommitRelation::Merged;

        fn with_parents(
            mut s: but_core::ref_metadata::WorkspaceStack,
            assignments: &[(&str, &[&str])],
        ) -> but_core::ref_metadata::WorkspaceStack {
            for (name, parents) in assignments {
                let idx = s
                    .branches
                    .iter()
                    .position(|b| b.ref_name.as_ref() == r(name))
                    .expect("assigned branch exists");
                s.branches[idx].parents = Some(parents.iter().map(|p| r(p).to_owned()).collect());
            }
            s
        }

        #[test]
        fn chains_always_pass_even_with_duplicates() {
            let s = stack(1, Merged, ["refs/heads/A", "refs/heads/B", "refs/heads/A"]);
            assert!(s.validate_structure().is_ok(), "legacy tolerance holds");
            assert_eq!(s.parent_edges(), [(0, 1), (1, 2)]);
        }

        #[test]
        fn diamond_is_valid_and_yields_its_edges() {
            // tip M rests on both arms, which rest on base.
            let s = with_parents(
                stack(
                    1,
                    Merged,
                    [
                        "refs/heads/M",
                        "refs/heads/left",
                        "refs/heads/right",
                        "refs/heads/base",
                    ],
                ),
                &[
                    ("refs/heads/M", &["refs/heads/left", "refs/heads/right"]),
                    ("refs/heads/left", &["refs/heads/base"]),
                    ("refs/heads/right", &["refs/heads/base"]),
                    ("refs/heads/base", &[]),
                ],
            );
            s.validate_structure().unwrap();
            assert_eq!(s.parent_edges(), [(0, 1), (0, 2), (1, 3), (2, 3)]);
        }

        #[test]
        fn structural_violations_are_hard_errors() {
            let unknown = with_parents(
                stack(1, Merged, ["refs/heads/A", "refs/heads/B"]),
                &[("refs/heads/A", &["refs/heads/nope"])],
            );
            assert!(unknown.validate_structure().is_err(), "unknown parent");

            let upward = with_parents(
                stack(1, Merged, ["refs/heads/A", "refs/heads/B"]),
                &[("refs/heads/B", &["refs/heads/A"])],
            );
            assert!(upward.validate_structure().is_err(), "parent above child");

            let selfish = with_parents(
                stack(1, Merged, ["refs/heads/A", "refs/heads/B"]),
                &[("refs/heads/A", &["refs/heads/A"])],
            );
            assert!(selfish.validate_structure().is_err(), "self parent");

            let twice = with_parents(
                stack(1, Merged, ["refs/heads/A", "refs/heads/B"]),
                &[("refs/heads/A", &["refs/heads/B", "refs/heads/B"])],
            );
            assert!(twice.validate_structure().is_err(), "duplicate parent");

            let two_tips = with_parents(
                stack(
                    1,
                    Merged,
                    ["refs/heads/A", "refs/heads/B", "refs/heads/base"],
                ),
                &[
                    ("refs/heads/A", &["refs/heads/base"]),
                    ("refs/heads/B", &["refs/heads/base"]),
                    ("refs/heads/base", &[]),
                ],
            );
            assert!(two_tips.validate_structure().is_err(), "two tips");

            let dup_name = with_parents(
                stack(1, Merged, ["refs/heads/A", "refs/heads/B", "refs/heads/A"]),
                &[("refs/heads/A", &["refs/heads/B"])],
            );
            assert!(dup_name.validate_structure().is_err(), "duplicate names");
        }

        #[test]
        fn fork_insert_and_merge_parent_ops() {
            let mut s = stack(1, Merged, ["refs/heads/top", "refs/heads/base"]);
            s.add_fork(r("refs/heads/side").to_owned(), r("refs/heads/base"))
                .unwrap();
            assert_eq!(
                s.branches
                    .iter()
                    .map(|b| b.ref_name.to_string())
                    .collect::<Vec<_>>(),
                ["refs/heads/top", "refs/heads/side", "refs/heads/base"],
            );
            // top still (implicitly) rests on side?? NO: implied adjacency now points at
            // side — materialize the truth: the fork insertion may not corrupt top's
            // parent, so validate catches nothing but the EDGE moved. Assert the edges.
            assert_eq!(s.parent_edges(), [(0, 1), (1, 2)]);

            s.insert_on_edge(
                r("refs/heads/mid").to_owned(),
                r("refs/heads/side"),
                r("refs/heads/base"),
            )
            .unwrap();
            assert_eq!(s.parent_edges(), [(0, 1), (1, 2), (2, 3)]);

            s.add_merge_parent(r("refs/heads/top"), r("refs/heads/base"))
                .unwrap();
            s.validate_structure().unwrap();
            assert!(
                s.add_merge_parent(r("refs/heads/base"), r("refs/heads/top"))
                    .is_err(),
                "merge parent above the branch is rejected"
            );
            assert!(
                s.insert_on_edge(
                    r("refs/heads/x").to_owned(),
                    r("refs/heads/top"),
                    r("refs/heads/mid"),
                )
                .is_err(),
                "no such edge"
            );
        }

        #[test]
        fn removal_splices_children_onto_the_removed_ones_parents() {
            let mut ws = super::workspace(vec![with_parents(
                stack(
                    1,
                    Merged,
                    [
                        "refs/heads/M",
                        "refs/heads/left",
                        "refs/heads/right",
                        "refs/heads/base",
                    ],
                ),
                &[
                    ("refs/heads/M", &["refs/heads/left", "refs/heads/right"]),
                    ("refs/heads/left", &["refs/heads/base"]),
                    ("refs/heads/right", &["refs/heads/base"]),
                    ("refs/heads/base", &[]),
                ],
            )]);
            assert!(ws.remove_segment(r("refs/heads/left")));
            let s = &ws.stacks[0];
            s.validate_structure().unwrap();
            assert_eq!(
                s.branches[0].parents.as_ref().unwrap()[0].to_string(),
                "refs/heads/base",
                "M inherited left's parent in left's slot"
            );
            assert_eq!(
                s.parent_edges(),
                [(0, 2), (0, 1), (1, 2)],
                "base sits in left's old slot, then right, and right still rests on base"
            );
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
                    parents: None,
                })
                .collect(),
            workspacecommit_relation,
        }
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
