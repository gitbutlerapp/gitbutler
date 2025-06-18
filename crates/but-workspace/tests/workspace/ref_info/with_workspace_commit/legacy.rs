mod stacks {
    use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;
    use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack};
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::{StacksFilter, stacks_v3};
    use gitbutler_stack::StackId;

    #[test]
    fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
    -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;

        add_stack(
            &mut meta,
            StackId::from_number_for_testing(1),
            "B-on-A",
            StackState::InWorkspace,
        );
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(2),
            "C-on-A",
            StackState::Inactive,
        );
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(3),
            "does-not-exist-inactive",
            StackState::Inactive,
        );
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(4),
            "does-not-exist-active",
            StackState::InWorkspace,
        );
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        *   820f2b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |\  
        | * 4e5484a (B-on-A) add new file in B-on-A
        * | 5f37dbf (C-on-A) add new file in C-on-A
        |/  
        | * 89cc2d3 (origin/A) change in A
        |/  
        * d79bba9 (A) new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");
        let actual = stacks_v3(&repo, &meta, StacksFilter::All)?;
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: 00000000-0000-0000-0000-000000000002,
                heads: [
                    StackHeadInfo {
                        name: "C-on-A",
                        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    },
                ],
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                order: None,
            },
            StackEntry {
                id: 00000000-0000-0000-0000-000000000001,
                heads: [
                    StackHeadInfo {
                        name: "B-on-A",
                        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                    },
                ],
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                order: None,
            },
        ]
        "#);

        let actual = stacks_v3(&repo, &meta, StacksFilter::InWorkspace)?;
        // It lists both still as both are reachable from a workspace commit, so clearly in the workspace.
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: 00000000-0000-0000-0000-000000000002,
                heads: [
                    StackHeadInfo {
                        name: "C-on-A",
                        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    },
                ],
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                order: None,
            },
            StackEntry {
                id: 00000000-0000-0000-0000-000000000001,
                heads: [
                    StackHeadInfo {
                        name: "B-on-A",
                        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                    },
                ],
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                order: None,
            },
        ]
        "#);

        let actual = stacks_v3(&repo, &meta, StacksFilter::Unapplied)?;
        // nothing reachable
        insta::assert_debug_snapshot!(actual, @"[]");

        add_stack(
            &mut meta,
            StackId::from_number_for_testing(5),
            "main",
            StackState::Inactive,
        );

        let actual = stacks_v3(&repo, &meta, StacksFilter::Unapplied)?;
        // Still nothing reachable
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: 00000000-0000-0000-0000-000000000005,
                heads: [
                    StackHeadInfo {
                        name: "main",
                        tip: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    },
                ],
                tip: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                order: None,
            },
        ]
        "#);
        Ok(())
    }
}

mod stack_details {
    use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;
    use crate::ref_info::with_workspace_commit::utils::{
        StackState, add_stack, add_stack_with_segments,
    };
    use but_testsupport::visualize_commit_graph_all;
    use gitbutler_stack::StackId;

    #[test]
    fn simple_fully_pushed() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario(
            "three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant",
        )?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cbc6713 (origin/advanced-lane, on-top-of-dependant, dependant, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

        let stack_id = add_stack_with_segments(
            &mut meta,
            StackId::from_number_for_testing(1),
            "dependant",
            StackState::InWorkspace,
            &["advanced-lane"],
        );
        let actual = but_workspace::stack_details_v3(stack_id, &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "dependant",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "dependant",
                    remote_tracking_branch: None,
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    base_commit: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    push_status: CompletelyUnpushed,
                    last_updated_at: Some(
                        0,
                    ),
                    authors: [],
                    is_conflicted: false,
                    commits: [],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "advanced-lane",
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/advanced-lane",
                    ),
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    base_commit: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    push_status: NothingToPush,
                    last_updated_at: Some(
                        0,
                    ),
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(cbc6713, "change", local/remote(identity)),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
            ],
            is_conflicted: false,
        }
        "#);
        Ok(())
    }

    #[test]
    fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
    -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        *   820f2b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |\  
        | * 4e5484a (B-on-A) add new file in B-on-A
        * | 5f37dbf (C-on-A) add new file in C-on-A
        |/  
        | * 89cc2d3 (origin/A) change in A
        |/  
        * d79bba9 (A) new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");

        let b_stack_id = add_stack(
            &mut meta,
            StackId::from_number_for_testing(1),
            "B-on-A",
            StackState::InWorkspace,
        );
        let actual = but_workspace::stack_details_v3(b_stack_id, &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "B-on-A",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "B-on-A",
                    remote_tracking_branch: None,
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    push_status: CompletelyUnpushed,
                    last_updated_at: Some(
                        0,
                    ),
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(4e5484a, "add new file in B-on-A", local),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
            ],
            is_conflicted: false,
        }
        "#);

        let c_stack_id = add_stack(
            &mut meta,
            StackId::from_number_for_testing(2),
            "C-on-A",
            StackState::InWorkspace,
        );
        let actual = but_workspace::stack_details_v3(c_stack_id, &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "C-on-A",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "C-on-A",
                    remote_tracking_branch: None,
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                    base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    push_status: CompletelyUnpushed,
                    last_updated_at: Some(
                        0,
                    ),
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(5f37dbf, "add new file in C-on-A", local),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "A",
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    push_status: NothingToPush,
                    last_updated_at: None,
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(d79bba9, "new file in A", local/remote(identity)),
                    ],
                    upstream_commits: [
                        UpstreamCommit(89cc2d3, "change in A"),
                    ],
                    is_remote_head: false,
                },
            ],
            is_conflicted: false,
        }
        "#);
        Ok(())
    }
}
