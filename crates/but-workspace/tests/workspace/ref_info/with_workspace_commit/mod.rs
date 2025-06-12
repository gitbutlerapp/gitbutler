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
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_dependent_branches_rebased_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-rebased-with-remotes")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e26f4fd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 31b3f92 (B-on-A) change in B
    * 51db0ec (A) change after push
    | * ec39463 (origin/B-on-A) change in B
    |/  
    * 807f596 (origin/A) change in A
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B-on-A",
        StackState::InWorkspace,
        &["A"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits_unique_from_tip: [
                            LocalCommit(31b3f92, "change in B\n", local/remote(similarity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(51db0ec, "change after push\n", local),
                            LocalCommit(807f596, "change in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_dependent_branches_rebased_explicit_remote_in_extra_segment() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "two-dependent-branches-rebased-explicit-remote-in-extra-segment",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e26f4fd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 31b3f92 (B-on-A) change in B
    * 51db0ec (A) change after push
    | * ec39463 (origin/B-on-A) change in B
    |/  
    * 807f596 (origin/A, base-of-A) change in A
    * fafd9d0 (origin/main, main) init
    ");

    // Note how `base-of-A` is absent, it's just something the user may have added,
    // and it comes with an official remote configuration.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B-on-A",
        StackState::InWorkspace,
        &["A"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits_unique_from_tip: [
                            LocalCommit(31b3f92, "change in B\n", local/remote(similarity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(51db0ec, "change after push\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/base-of-A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(807f596, "change in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
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
fn two_dependent_branches_first_merged_no_ff() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-first-merge-no-ff")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   a455fe7 (origin/main) Merge branch 'A' into new-origin-main
    |\  
    | | * 4a62dfc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | * de11c03 (origin/B-on-A, B-on-A) change in B
    | |/  
    | * 0ee3a9e (origin/A, A) change in A
    |/  
    * fafd9d0 (main) init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B-on-A",
        StackState::InWorkspace,
        &["A"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits_unique_from_tip: [
                            LocalCommit(de11c03, "change in B\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(0ee3a9e, "change in A\n", integrated),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_dependent_branches_first_merged_no_ff_second_merged_on_remote_into_base_branch_integration_caught_up()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "two-dependent-branches-first-merge-no-ff-second-merge-into-first-on-remote",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   059cc4f (origin/A) Merge branch 'B-on-A' into new-origin-A
    |\  
    | | *   a455fe7 (origin/main, main) Merge branch 'A' into new-origin-main
    | | |\  
    | |_|/  
    |/| |   
    | | | * 4a62dfc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | | |/  
    | |/|   
    | * | de11c03 (origin/B-on-A, B-on-A) change in B
    |/ /  
    * / 0ee3a9e (A) change in A
    |/  
    * fafd9d0 init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "B-on-A",
        StackState::InWorkspace,
        &["A"],
    );

    // TODO: A must be considered integrated, and ideally still has its commits.
    //       Having commits would mean we still know the previous position of
    //       the local integration branch, which now has caught up and we have to
    //       determine (with or without displaying commits, that we are actually integrated.
    //       In this case, the post-cleanup re-adds a branch we otherwise didn't see, and we
    //       should probably have it do some sort of integration check based on the commit.
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
                    Sha1(0ee3a9e12c17b59a8507bbfe2ae98ab362feb21a),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits_unique_from_tip: [
                            LocalCommit(de11c03, "change in B\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(0ee3a9e, "change in A\n", integrated),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(059cc4f, "Merge branch \'B-on-A\' into new-origin-A\n",
                        ],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn target_ahead_remote_rewritten() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("target-ahead-remote-rewritten")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 03d2336 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * d5d3a92 (A) unique local tip
    * 6ffd040 shared by name
    * 4cd56ab unique local
    | * f4ed16f (origin/main) target ahead
    | | * 50d31c8 (origin/A) unique remote
    | | * a9954f1 shared by name
    | |/  
    |/|   
    * | 872c22f shared local/remote
    |/  
    * c166d42 (origin/main, origin/HEAD, main) init-integration
    ");

    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "A",
        StackState::InWorkspace,
    );
    let opts = standard_options();
    let info = ref_info(repo.find_reference("A")?, &*meta, opts)?;
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
                        commits_unique_from_tip: [
                            LocalCommit(d5d3a92, "unique local tip\n", local),
                            LocalCommit(6ffd040, "shared by name\n", local/remote(similarity)),
                            LocalCommit(4cd56ab, "unique local\n", local),
                            LocalCommit(872c22f, "shared local/remote\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(50d31c8, "unique remote\n",
                        ],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn single_commit_but_two_branches_one_in_ws_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-branches-one-advanced-one-parent-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   7f3248e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (advanced-lane) change
    * | 93d7eac (advanced-lane-2) change 2
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    for (idx, name) in ["lane", "advanced-lane-2", "advanced-lane"]
        .into_iter()
        .enumerate()
    {
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(idx as u128),
            name,
            StackState::InWorkspace,
        );
    }

    let opts = standard_options();
    let info = head_info(&repo, &*meta, opts)?;
    // The difficulty here is that there is no merge-parent for the newly created stack, and that
    // empty stacks rest on the merge-base which naturally is hidden during traversal.
    // Also, according to
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane-2",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(93d7eac, "change 2\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn single_commit_but_two_branches_one_in_ws_commit_with_virtual_segments() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("multiple-dependent-branches-per-stack-without-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * cbc6713 (HEAD -> gitbutler/workspace, lane) change
    * fafd9d0 (origin/main, main, lane-segment-02, lane-segment-01, lane-2-segment-02, lane-2-segment-01, lane-2) init
    ");

    // Follow the natural order, lane first.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(1),
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );

    // The stacks should come out just like defined above, "lane" and then "lane2" with all the right segments.
    let opts = standard_options();
    let info = ref_info(repo.find_reference("lane")?, &*meta, opts)?;
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-segment-01",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-segment-02",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane-2",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-2-segment-01",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-2-segment-02",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    // Natural order here is `lane` first, but we say we want `lane-2` first
    meta.data_mut().branches.clear();
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(1),
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );

    let opts = standard_options();
    let info = ref_info(repo.find_reference("lane")?, &*meta, opts)?;
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane-2",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-2-segment-01",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-2-segment-02",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-segment-01",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane-segment-02",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn single_commit_but_two_branches_both_in_ws_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-branches-one-advanced-two-parent-ws-commit")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (advanced-lane) change
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    for (idx, name) in ["advanced-lane", "lane"].into_iter().enumerate() {
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(idx as u128),
            name,
            StackState::InWorkspace,
        );
    }

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn single_commit_pushed_but_two_branches_both_in_ws_commit() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (origin/advanced-lane, advanced-lane) change
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    // For complexity, we also don't set up any branch metadata, only 'something' to get the target ref.
    add_workspace(&mut meta);
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
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
fn single_commit_pushed_but_two_branches_both_in_ws_commit_empty_dependant() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   335d6f2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * cbc6713 (origin/advanced-lane, dependant, advanced-lane) change
    |/  
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "dependant",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    // Put it below - this is fine, new commits will the placed onto `base`.
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "advanced-lane",
        StackState::InWorkspace,
        &["dependant"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn single_commit_pushed_ws_commit_empty_dependant() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cbc6713 (origin/advanced-lane, on-top-of-dependant, dependant, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "on-top-of-dependant",
        StackState::InWorkspace,
        &["dependant", "advanced-lane"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/on-top-of-dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    meta.data_mut().branches.clear();
    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "dependant",
        StackState::InWorkspace,
        &["on-top-of-dependant", "advanced-lane"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/on-top-of-dependant",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_branches_stracked_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-with-one-commit-with-remotes")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9b3cfd4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 788ad06 (origin/on-top-of-lane, on-top-of-lane) change on top
    * cbc6713 (origin/lane, lane) change
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(
        &mut meta,
        StackId::from_number_for_testing(0),
        "on-top-of-lane",
        StackState::InWorkspace,
        &["lane"],
    );

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/on-top-of-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/on-top-of-lane",
                        commits_unique_from_tip: [
                            LocalCommit(788ad06, "change on top\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "refs/remotes/origin/lane",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_branches_stacked_with_interesting_remote_setup() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-with-interesting-remote-setup")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a221221 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * aadad9d (A) shared by name
    * 96a2408 (origin/main) another unrelated
    | * 2b1808c (origin/A) shared by name
    |/  
    * f15ca75 (integrated) other integrated
    * 9456d79 integrated in target
    * fafd9d0 (main) init
    ");

    // Just a single explicit reference we want to know of.
    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "A",
        StackState::InWorkspace,
    );

    let opts = standard_options();
    let info = ref_info(repo.find_reference("A")?, &*meta, opts).unwrap();

    // TODO: A should just have 1 commit, remote should only have one commit.
    //       'integrated' shouldn't be listed.
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(aadad9d, "shared by name\n", local/remote(similarity)),
                            LocalCommit(96a2408, "another unrelated\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(f15ca75, "other integrated\n",
                            RemoteCommit(9456d79, "integrated in target\n",
                        ],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/integrated",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(f15ca75, "other integrated\n", integrated),
                            LocalCommit(9456d79, "integrated in target\n", integrated),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
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
fn single_commit_but_two_branches_stack_on_top_of_ws_commit() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-branches-one-advanced-ws-commit-on-top-of-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * cbc6713 (HEAD -> gitbutler/workspace, advanced-lane) change
    * fafd9d0 (origin/main, main, lane) init
    ");

    for (idx, name) in ["advanced-lane", "lane"].into_iter().enumerate() {
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(idx as u128),
            name,
            StackState::InWorkspace,
        );
    }

    let opts = standard_options();
    let err = head_info(&repo, &*meta, opts).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Workspace reference gitbutler/workspace didn't point to a managed commit anymore",
        "we simply don't know what to do here yet"
    );

    let info = ref_info(repo.find_reference("advanced-lane")?, &*meta, opts).unwrap();
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn two_branches_one_advanced_two_parent_ws_commit_diverged_remote_tracking_branch()
-> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   873d056 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | cbc6713 (advanced-lane) change
    |/  
    * fafd9d0 (main, lane) init
    * da83717 (origin/main) disjoint remote target
    ");

    for (idx, name) in ["lane", "advanced-lane"].into_iter().enumerate() {
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(idx as u128),
            name,
            StackState::InWorkspace,
        );
    }

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    let info = ref_info(repo.find_reference("advanced-lane")?, &*meta, opts)?;
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    let info = ref_info(repo.find_reference("lane")?, &*meta, opts)?;
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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

    meta.data_mut().branches.clear();
    for (idx, name) in ["advanced-lane", "lane"].into_iter().enumerate() {
        add_stack(
            &mut meta,
            StackId::from_number_for_testing(idx as u128),
            name,
            StackState::InWorkspace,
        );
    }

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
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                ],
                stash_status: None,
            },
            Stack {
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
fn disjoint() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("disjoint")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 32791d2 (HEAD -> disjoint) disjoint init
    * fafd9d0 (origin/main, main) init
    ");

    add_stack(
        &mut meta,
        StackId::from_number_for_testing(1),
        "disjoint",
        StackState::InWorkspace,
    );

    let opts = standard_options();
    let info = head_info(&repo, &*meta, opts)?;

    // We see the commit in the branch as there is no base to hide it.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                base: None,
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/disjoint",
                        remote_tracking_ref_name: "None",
                        commits_unique_from_tip: [
                            LocalCommit(32791d2, "disjoint init\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
                        commits_unique_from_tip: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: "None",
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
                        commits_unique_from_tip: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
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
                        commits_unique_from_tip: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
                    },
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: "None",
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
                        commits_unique_from_tip: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
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
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: "None",
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
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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
        stacks: [],
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
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: Branch {
                            ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                            description: None,
                            review: Review { pull_request: None, review_id: None },
                        },
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

mod legacy;

mod branch_details {
    use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::branch_details_v3;

    #[test]
    fn disjoint() -> anyhow::Result<()> {
        let (repo, meta) = read_only_in_memory_scenario("disjoint")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 32791d2 (HEAD -> disjoint) disjoint init
    * fafd9d0 (origin/main, main) init
    ");

        let actual = branch_details_v3(&repo, "refs/heads/disjoint".try_into()?, &*meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        BranchDetails {
            name: "refs/heads/disjoint",
            remote_tracking_branch: None,
            description: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(32791d22e276ec0ed87d14f906321137356bc6d6),
            base_commit: Sha1(32791d22e276ec0ed87d14f906321137356bc6d6),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
                committer <committer@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(32791d2, "disjoint init", local/remote(identity)),
            ],
            upstream_commits: [],
            is_remote_head: false,
        }
        "#);

        let actual = branch_details_v3(&repo, "refs/heads/main".try_into()?, &*meta)?;
        insta::assert_debug_snapshot!(actual, @r#"
        BranchDetails {
            name: "refs/heads/main",
            remote_tracking_branch: None,
            description: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
            base_commit: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [],
            is_conflicted: false,
            commits: [],
            upstream_commits: [],
            is_remote_head: false,
        }
        "#);

        Ok(())
    }
}
mod utils {
    use crate::ref_info::utils::named_read_only_in_memory_scenario;
    use but_graph::VirtualBranchesTomlMetadata;
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

    pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack(
            meta,
            StackId::from_number_for_testing(u128::MAX),
            "definitely outside of the workspace just to have it",
            StackState::Inactive,
        );
    }

    pub fn add_stack(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: StackId,
        stack_name: &str,
        state: StackState,
    ) -> StackId {
        add_stack_with_segments(meta, stack_id, stack_name, state, &[])
    }

    // Add parameters as needed.
    pub fn add_stack_with_segments(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: StackId,
        stack_name: &str,
        state: StackState,
        segments: &[&str],
    ) -> StackId {
        let mut stack = gitbutler_stack::Stack::new_with_just_heads(
            segments
                .iter()
                .rev()
                .map(|stack_name| {
                    gitbutler_stack::StackBranch::new_with_zero_head(
                        (*stack_name).into(),
                        None,
                        None,
                        None,
                        false,
                    )
                })
                .chain(std::iter::once(
                    gitbutler_stack::StackBranch::new_with_zero_head(
                        stack_name.into(),
                        None,
                        None,
                        None,
                        false,
                    ),
                ))
                .collect(),
            0,
            meta.data().branches.len(),
            match state {
                StackState::InWorkspace => true,
                StackState::Inactive => false,
            },
        );
        stack.id = stack_id;
        meta.data_mut().branches.insert(stack_id, stack);
        stack_id
    }
}
use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, add_workspace,
};
use utils::add_stack;
pub use utils::read_only_in_memory_scenario;
