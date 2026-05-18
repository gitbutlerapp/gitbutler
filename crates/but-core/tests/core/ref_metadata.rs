mod workspace {
    use but_core::ref_metadata::{
        StackId,
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
        let ws = Workspace {
            stacks: vec![
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
            ],
            ..Workspace::default()
        };

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

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known ref")
    }
    fn new_stack_id(_: &gix::refs::FullNameRef) -> StackId {
        StackId::generate()
    }
}
