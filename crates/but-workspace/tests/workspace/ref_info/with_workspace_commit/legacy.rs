mod stacks {
    use crate::ref_info::with_workspace_commit::{
        read_only_in_memory_scenario,
        utils::{StackState, add_stack},
    };
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::legacy::StacksFilter;
    use but_workspace::{legacy::stack_details_v3, legacy::stacks_v3};

    #[test]
    fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
    -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;

        add_stack(&mut meta, 1, "B-on-A", StackState::InWorkspace);
        add_stack(&mut meta, 2, "C-on-A", StackState::Inactive);
        add_stack(
            &mut meta,
            3,
            "does-not-exist-inactive",
            StackState::Inactive,
        );
        add_stack(
            &mut meta,
            4,
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
        // It's notable that the segment A is shared between both stacks.
        let actual = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000002,
                ),
                heads: [
                    StackHeadInfo {
                        name: "C-on-A",
                        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        is_checked_out: false,
                    },
                ],
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                order: None,
                is_checked_out: false,
            },
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                heads: [
                    StackHeadInfo {
                        name: "B-on-A",
                        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        is_checked_out: false,
                    },
                ],
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                order: None,
                is_checked_out: false,
            },
        ]
        "#);

        let actual = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
        // It lists both still as both are reachable from a workspace commit, so clearly in the workspace.
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000002,
                ),
                heads: [
                    StackHeadInfo {
                        name: "C-on-A",
                        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        is_checked_out: false,
                    },
                ],
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                order: None,
                is_checked_out: false,
            },
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                heads: [
                    StackHeadInfo {
                        name: "B-on-A",
                        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        is_checked_out: false,
                    },
                ],
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                order: None,
                is_checked_out: false,
            },
        ]
        "#);

        let actual = stacks_v3(
            &repo,
            &meta,
            StacksFilter::InWorkspace,
            Some("refs/heads/A".try_into()?),
        )?;
        // Now it's seen as 'checked-out' as a sign for the UI to do something differently.
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000002,
                ),
                heads: [
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        is_checked_out: true,
                    },
                ],
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                order: None,
                is_checked_out: true,
            },
        ]
        "#);

        let details = stack_details_v3(actual[0].id, &repo, &meta)?;
        // This still returns the whole stack,
        // as it relies on checking the actual HEAD reference to know what's checked out and what to
        // filter.
        insta::assert_debug_snapshot!(details, @r#"
        StackDetails {
            derived_name: "C-on-A",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "C-on-A",
                    linked_worktree_id: None,
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
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    push_status: UnpushedCommitsRequiringForce,
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

        let actual = stacks_v3(&repo, &meta, StacksFilter::Unapplied, None)?;
        // nothing reachable
        insta::assert_debug_snapshot!(actual, @"[]");

        add_stack(&mut meta, 5, "main", StackState::Inactive);

        let actual = stacks_v3(&repo, &meta, StacksFilter::Unapplied, None)?;
        // Still nothing reachable
        insta::assert_debug_snapshot!(actual, @r#"
        [
            StackEntry {
                id: Some(
                    00000000-0000-0000-0000-000000000005,
                ),
                heads: [
                    StackHeadInfo {
                        name: "main",
                        tip: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                        is_checked_out: false,
                    },
                ],
                tip: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                order: None,
                is_checked_out: false,
            },
        ]
        "#);
        Ok(())
    }
}

mod stack_details {
    use crate::ref_info::with_workspace_commit::{
        read_only_in_memory_scenario,
        utils::{StackState, add_stack, add_stack_with_segments},
    };
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::legacy::stack_details_v3;

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
            1,
            "dependant",
            StackState::InWorkspace,
            &["advanced-lane"],
        );
        let actual = stack_details_v3(stack_id.into(), &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "dependant",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "dependant",
                    linked_worktree_id: None,
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
                    linked_worktree_id: None,
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

        let b_stack_id = add_stack(&mut meta, 1, "B-on-A", StackState::InWorkspace);
        let c_stack_id = add_stack(&mut meta, 2, "C-on-A", StackState::InWorkspace);
        let actual = stack_details_v3(Some(b_stack_id), &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "B-on-A",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "B-on-A",
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
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
                        Commit(4e5484a, "add new file in B-on-A", local),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "A",
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    push_status: UnpushedCommitsRequiringForce,
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

        let actual = stack_details_v3(Some(c_stack_id), &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "C-on-A",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "C-on-A",
                    linked_worktree_id: None,
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
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
                    description: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                    push_status: UnpushedCommitsRequiringForce,
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
