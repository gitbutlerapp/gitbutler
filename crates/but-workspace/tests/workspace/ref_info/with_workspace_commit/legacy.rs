mod stacks {
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::legacy::StacksFilter;

    use crate::ref_info::{
        stack_details_v3, stacks_v3,
        with_workspace_commit::{
            read_only_in_memory_scenario,
            utils::{StackState, add_stack},
        },
    };

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
                        review_id: None,
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        review_id: None,
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
                        review_id: None,
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        review_id: None,
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
                        review_id: None,
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        review_id: None,
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
                        review_id: None,
                        is_checked_out: false,
                    },
                    StackHeadInfo {
                        name: "A",
                        tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        review_id: None,
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
                        review_id: None,
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
                    reference: FullName(
                        "refs/heads/C-on-A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                    base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
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
                    reference: FullName(
                        "refs/heads/A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
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
                        review_id: None,
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
    use but_testsupport::{graph_workspace, invoke_bash, visualize_commit_graph_all};

    use crate::ref_info::{
        head_info, stack_details_v3,
        utils::standard_options,
        with_workspace_commit::{
            read_only_in_memory_scenario,
            utils::named_writable_scenario,
            utils::{StackState, add_stack, add_stack_with_segments},
        },
    };

    #[test]
    fn simple_fully_pushed() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario(
            "three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependent",
        )?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * cbc6713 (origin/advanced-lane, on-top-of-dependent, dependent, advanced-lane) change
        * fafd9d0 (origin/main, main, lane) init
        ");

        let stack_id = add_stack_with_segments(
            &mut meta,
            1,
            "dependent",
            StackState::InWorkspace,
            &["advanced-lane"],
        );
        let actual = stack_details_v3(stack_id.into(), &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "dependent",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "dependent",
                    reference: FullName(
                        "refs/heads/dependent",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    base_commit: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
                    authors: [],
                    is_conflicted: false,
                    commits: [],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "advanced-lane",
                    reference: FullName(
                        "refs/heads/advanced-lane",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/advanced-lane",
                    ),
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
                    base_commit: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    push_status: NothingToPush,
                    last_updated_at: None,
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
                    reference: FullName(
                        "refs/heads/B-on-A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                    base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
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
                    reference: FullName(
                        "refs/heads/A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
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
                    reference: FullName(
                        "refs/heads/C-on-A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                    base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
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
                    reference: FullName(
                        "refs/heads/A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/A",
                    ),
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

    #[test]
    fn multi_segment_stack_uses_advanced_tip_ref_to_find_full_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
        let stack_id = add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

        invoke_bash(
            r#"
            git checkout B
            git commit --allow-empty -m B-outside
            git checkout gitbutler/workspace
            "#,
            &repo,
        );

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * cc0bf57 (B) B-outside
        | * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |/  
        * d69fe94 B
        * 09d8e52 (A) A
        * 85efbe4 (origin/main, main) M
        ");

        // The raw workspace projection still knows about the advanced tip, but it cannot attach it
        // to `refs/heads/B` anymore from `HEAD`, so the top segment is already anonymous here.
        // Strangely enough, the worktree projection is absolutely supposed to be able to see that
        // if the stack tips are known to the workspace, but it simply doesn't see it here.
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            but_graph::init::Options {
                ..standard_options().traversal
            },
        )?;
        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
        └── ≡:5:anon: on 85efbe4 {1}
            ├── :5:anon:
            │   └── ·d69fe94 (🏘️)
            └── 📙:4:A
                └── ·09d8e52 (🏘️)
        ");
        insta::assert_debug_snapshot!(ws, @r#"
        Workspace(📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4) {
            id: 0,
            kind: Managed {
                ref_info: RefInfo {
                    ref_name: FullName(
                        "refs/heads/gitbutler/workspace",
                    ),
                    worktree: Some(
                        Main,
                    ),
                },
            },
            stacks: [
                Stack(≡:5:anon: on 85efbe4 {1}) {
                    segments: [
                        StackSegment(:5:anon:) {
                            commits: [
                                "·d69fe94 (🏘\u{fe0f})",
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                        },
                        StackSegment(📙:4:A) {
                            commits: [
                                "·09d8e52 (🏘\u{fe0f})",
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                        },
                    ],
                    id: 00000000-0000-0000-0000-000000000001,
                },
            ],
            metadata: Some(
                Workspace {
                    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
                    stacks: [
                        WorkspaceStack {
                            id: 00000000-0000-0000-0000-000000000001,
                            branches: [
                                WorkspaceStackBranch {
                                    ref_name: "refs/heads/B",
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
                    target_ref: "refs/remotes/origin/main",
                    target_commit_id: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
                    push_remote: None,
                },
            ),
            target_ref: Some(
                TargetRef {
                    ref_name: FullName(
                        "refs/remotes/origin/main",
                    ),
                    segment_index: NodeIndex(1),
                    commits_ahead: 0,
                },
            ),
            extra_target: None,
        }
        "#);

        // Looking from `HEAD` means traversing the workspace ref, so `B-outside` is not part of the
        // traversed graph at all.
        // The stack is still recognized from metadata, but the advanced tip can no longer be attached
        // to `refs/heads/B`, so the top segment becomes anonymous and there are no `commits_outside`
        // to report here. Those are only populated for commits that are visible in the traversal but
        // sit above the managed workspace commit.
        // This *should not be*, it should detect this case.
        let info = head_info(&repo, &meta, standard_options())?;
        insta::assert_debug_snapshot!(info, @r#"
        RefInfo {
            workspace_ref_info: Some(
                RefInfo {
                    ref_name: FullName(
                        "refs/heads/gitbutler/workspace",
                    ),
                    worktree: Some(
                        Main,
                    ),
                },
            ),
            symbolic_remote_names: {
                "origin",
            },
            stacks: [
                Stack {
                    id: Some(
                        00000000-0000-0000-0000-000000000001,
                    ),
                    base: Some(
                        Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(5),
                            ref_name: "None",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(d69fe94, "B\n", local),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                            base: "09d8e52",
                        },
                        ref_info::ui::Segment {
                            id: NodeIndex(4),
                            ref_name: "►A",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(09d8e52, "A\n", local),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                            metadata: Branch,
                            push_status: CompletelyUnpushed,
                            base: "85efbe4",
                        },
                    ],
                },
            ],
            target_ref: Some(
                TargetRef {
                    ref_name: FullName(
                        "refs/remotes/origin/main",
                    ),
                    segment_index: NodeIndex(1),
                    commits_ahead: 0,
                },
            ),
            target_commit: Some(
                TargetCommit {
                    commit_id: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
                    segment_index: NodeIndex(2),
                },
            ),
            extra_target: None,
            lower_bound: Some(
                NodeIndex(2),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            ancestor_workspace_commit: None,
            is_entrypoint: true,
        }
        "#);

        // Looking up by `stack_id` takes a different path: it first finds a surviving ref for the
        // stack, here `refs/heads/B`, and calls `ref_info()` from that ref instead of from `HEAD`.
        // From that starting point `B-outside` is no longer "outside" at all, but simply the tip
        // commit of the `B` segment, so it shows up in `commits` rather than `commits_outside`.
        // The legacy `StackDetails` projection also drops `commits_outside` entirely, so this view
        // cannot distinguish the advanced commit from ordinary in-stack commits anymore.
        let actual = stack_details_v3(Some(stack_id), &repo, &meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        StackDetails {
            derived_name: "B",
            push_status: CompletelyUnpushed,
            branch_details: [
                BranchDetails {
                    name: "B",
                    reference: FullName(
                        "refs/heads/B",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(cc0bf57992f34345564a4f616f60dd880cd83377),
                    base_commit: Sha1(09d8e528cc9381ddc4a7a436d83507b20fc909b0),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(cc0bf57, "B-outside", local),
                        Commit(d69fe94, "B", local),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "A",
                    reference: FullName(
                        "refs/heads/A",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: None,
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(09d8e528cc9381ddc4a7a436d83507b20fc909b0),
                    base_commit: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
                    push_status: CompletelyUnpushed,
                    last_updated_at: None,
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(09d8e52, "A", local),
                    ],
                    upstream_commits: [],
                    is_remote_head: false,
                },
                BranchDetails {
                    name: "main",
                    reference: FullName(
                        "refs/heads/main",
                    ),
                    linked_worktree_id: None,
                    remote_tracking_branch: Some(
                        "refs/remotes/origin/main",
                    ),
                    pr_number: None,
                    review_id: None,
                    tip: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
                    base_commit: Sha1(0000000000000000000000000000000000000000),
                    push_status: Integrated,
                    last_updated_at: None,
                    authors: [
                        author <author@example.com>,
                    ],
                    is_conflicted: false,
                    commits: [
                        Commit(85efbe4, "M", integrated),
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
}
