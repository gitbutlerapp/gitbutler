use bstr::ByteSlice;
use but_testsupport::{visualize_commit_graph, visualize_commit_graph_all};
use but_workspace::{head_info, ref_info};
use gitbutler_stack::StackId;
use pretty_assertions::assert_eq;

#[test]
fn remote_ahead_fast_forwardable() -> anyhow::Result<()> {
    let (mut repo, mut meta) = read_only_in_memory_scenario("remote-advanced-ff")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * fb27086 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 89cc2d3 (origin/A) change in A
    |/  
    * d79bba9 (A) new file in A
    * c166d42 (origin/main, origin/HEAD, main) init-integration
    ");

    // Needs a branch for workspace implied by a branch with metadata.
    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "A",
        StackState::InWorkspace,
    );
    // We can look at a workspace ref directly (via HEAD)
    let opts = standard_options();
    let info = head_info(&repo, &*meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: Some(
                            Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                        ),
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    let expected_info = info;

    let at = repo.find_reference("refs/heads/A")?;
    let info = ref_info(at, &*meta, opts)?;
    assert_eq!(
        info, expected_info,
        "Information doesn't change just because the starting point is different"
    );

    // Remove remote configuration to have it deduce the remote.
    // This is from the times when GB wouldn't set up Git remotes, so it's for backward compatibility.
    repo.config_snapshot_mut()
        .remove_section("branch", info.stacks[0].name().unwrap().shorten().as_bstr());

    let at = repo.find_reference("refs/heads/A")?;
    let info = ref_info(at, &*meta, opts)?;
    assert_eq!(
        info, expected_info,
        "Information doesn't change, the remote is inferred"
    );
    Ok(())
}

#[test]
fn multiple_branches_with_shared_segment() -> anyhow::Result<()> {
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

    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "C-on-A",
        StackState::InWorkspace,
    );

    let opts = standard_options();
    let info = head_info(&repo, &*meta, opts)?;

    // The shared "A" segment is only used in the first stack, despite reachable from both.
    // The first stack which reaches the shared segment claims it.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Some(
                            Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                        ),
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    let c_info = ref_info(repo.find_reference("C-on-A")?, &*meta, opts)?;

    // A partial workspace is provided, but the entire workspace is known.
    insta::assert_debug_snapshot!(c_info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Some(
                            Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                        ),
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);
    assert_eq!(info.stacks[0], c_info.stacks[0]);

    let b_info = ref_info(repo.find_reference("B-on-A")?, &*meta, opts)?;

    // It's like the stack is part of the workspace, so "A" is only used once.
    insta::assert_debug_snapshot!(b_info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    assert_eq!(
        info.stacks[1].segments, b_info.stacks[0].segments,
        "It's like the stack is part of the workspace, so 'A' is only used once."
    );

    let a_info = ref_info(repo.find_reference("A")?, &*meta, opts)?;

    // We can also show segments that are part of the stack (like homing in on them), as long as they are in a workspace.
    insta::assert_debug_snapshot!(a_info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);
    Ok(())
}

#[test]
fn empty_workspace_with_branch_below() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("empty-workspace-with-branch-below")?;
    insta::assert_snapshot!(visualize_commit_graph(&repo, "HEAD")?, @r"
    * c7276fa (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c166d42 (origin/main, origin/HEAD, unrelated, main) init-integration
    ");

    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "unrelated",
        StackState::InWorkspace,
    );

    let opts = standard_options();
    let info = head_info(&repo, &*meta, opts)?;
    // Active branches we should see, but only "unrelated",
    // not any other branch that happens to point at that commit.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/unrelated",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Some(
                            Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                        ),
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    // Change the stack to be inactive, so it's not considered to be part of the workspace.
    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "unrelated",
        StackState::Inactive,
    );

    let info = head_info(&repo, &*meta, opts)?;
    // Now there should be no stack, it's an empty workspace.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    // But if it's requested directly, we should see it nonetheless.
    let info = ref_info(repo.find_reference("unrelated")?, &*meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/unrelated",
                        remote_tracking_ref_name: "None",
                        ref_location: "OutsideOfWorkspace",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Some(
                            Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                        ),
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);
    Ok(())
}

mod legacy {
    mod stacks {
        use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;
        use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack};
        use but_testsupport::visualize_commit_graph_all;
        use but_workspace::{StacksFilter, stacks_v3};
        use gitbutler_stack::StackId;

        #[test]
        fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
        -> anyhow::Result<()> {
            let (repo, mut meta) =
                read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;

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
                },
            ]
            "#);
            Ok(())
        }
    }

    mod stack_details {
        use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;
        use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack};
        use but_testsupport::visualize_commit_graph_all;
        use gitbutler_stack::StackId;

        #[test]
        fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
        -> anyhow::Result<()> {
            let (repo, mut meta) =
                read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;
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
                push_status: UnpushedCommits,
                branch_details: [
                    BranchDetails {
                        name: "B-on-A",
                        remote_tracking_branch: None,
                        description: None,
                        pr_number: None,
                        review_id: None,
                        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                        base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                        push_status: UnpushedCommits,
                        last_updated_at: Some(
                            0,
                        ),
                        authors: [
                            author <author@example.com>,
                        ],
                        is_conflicted: false,
                        commits: [
                            Commit(4e5484a, "add new file in B-on-A"),
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
                push_status: UnpushedCommits,
                branch_details: [
                    BranchDetails {
                        name: "C-on-A",
                        remote_tracking_branch: None,
                        description: None,
                        pr_number: None,
                        review_id: None,
                        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                        base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                        push_status: UnpushedCommits,
                        last_updated_at: Some(
                            0,
                        ),
                        authors: [
                            author <author@example.com>,
                        ],
                        is_conflicted: false,
                        commits: [
                            Commit(5f37dbf, "add new file in C-on-A"),
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
                        push_status: CompletelyUnpushed,
                        last_updated_at: None,
                        authors: [
                            author <author@example.com>,
                        ],
                        is_conflicted: false,
                        commits: [
                            Commit(d79bba9, "new file in A"),
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
}

mod utils {
    use crate::ref_info::utils::named_read_only_in_memory_scenario;
    use but_workspace::VirtualBranchesTomlMetadata;
    use gitbutler_stack::StackId;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("with-remotes-and-workspace", name)?;
        let vb = meta.data_mut();
        vb.default_target = Some(gitbutler_stack::Target {
            // For simplicity, we stick to the defaults.
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            // Not required
            remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
            sha: git2::Oid::zero(),
            push_remote_name: None,
        });
        Ok((repo, meta))
    }

    pub enum StackState {
        InWorkspace,
        Inactive,
    }

    // Add parameters as needed.
    pub fn add_stack(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: StackId,
        stack_name: &str,
        state: StackState,
    ) -> StackId {
        let mut stack = gitbutler_stack::Stack::new_with_just_heads(
            vec![gitbutler_stack::StackBranch::new_with_zero_head(
                stack_name.into(),
                None,
                None,
                None,
                true,
            )],
            0,
            0,
            match state {
                StackState::InWorkspace => true,
                StackState::Inactive => false,
            },
        );
        stack.id = stack_id;
        meta.data_mut().branches.insert(stack_id, stack);
        meta.data();
        stack_id
    }
}
use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::utils::StackState;
use utils::add_stack;
pub use utils::read_only_in_memory_scenario;
