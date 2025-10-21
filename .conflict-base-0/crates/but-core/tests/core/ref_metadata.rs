mod workspace {
    use but_core::ref_metadata::WorkspaceCommitRelation::Merged;
    use but_core::ref_metadata::{StackKind::AppliedAndUnapplied, Workspace};

    #[test]
    fn add_new_stack_if_not_present_journey() {
        let mut ws = Workspace::default();
        assert_eq!(ws.stacks.len(), 0);

        let a_ref = r("refs/heads/A");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, Some(100), Merged),
            (0, 0)
        );
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(a_ref, Some(200), Merged),
            (0, 0)
        );
        assert_eq!(ws.stacks.len(), 1);

        let b_ref = r("refs/heads/B");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(b_ref, Some(0), Merged),
            (0, 0)
        );
        assert_eq!(
            ws.stack_names(AppliedAndUnapplied).collect::<Vec<_>>(),
            [b_ref, a_ref]
        );

        let c_ref = r("refs/heads/C");
        assert_eq!(
            ws.add_or_insert_new_stack_if_not_present(c_ref, None, Merged),
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
        insta::assert_debug_snapshot!(ws, @r"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [],
            target_ref: None,
            push_remote: None,
        }
        ");
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
            ws.add_or_insert_new_stack_if_not_present(a_ref, None, Merged),
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
            ws.add_or_insert_new_stack_if_not_present(a_ref, None, Merged),
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
            push_remote: None,
        }
        "#);

        assert!(ws.remove_segment(b_ref));
        assert!(ws.remove_segment(a_ref));
        assert!(ws.remove_segment(c_ref));

        // Everything should be removed.
        insta::assert_debug_snapshot!(ws, @r"
        Workspace {
            ref_info: RefInfo { created_at: None, updated_at: None },
            stacks: [],
            target_ref: None,
            push_remote: None,
        }
        ");
    }

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known ref")
    }
}
