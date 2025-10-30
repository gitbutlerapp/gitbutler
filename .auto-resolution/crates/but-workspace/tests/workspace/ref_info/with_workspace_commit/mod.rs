use bstr::ByteSlice;
use but_testsupport::{visualize_commit_graph, visualize_commit_graph_all};
use but_workspace::RefInfo;

pub fn head_info(
    repo: &gix::Repository,
    meta: &but_graph::VirtualBranchesTomlMetadata,
    mut opts: but_workspace::ref_info::Options,
) -> anyhow::Result<RefInfo> {
    if opts.traversal.extra_target_commit_id.is_none() {
        opts.traversal.extra_target_commit_id = meta.data().default_target.as_ref().map(|t| t.sha);
    }
    but_workspace::head_info(repo, meta, opts)
}

pub fn ref_info(
    existing_ref: gix::Reference<'_>,
    meta: &but_graph::VirtualBranchesTomlMetadata,
    mut opts: but_workspace::ref_info::Options,
) -> anyhow::Result<RefInfo> {
    if opts.traversal.extra_target_commit_id.is_none() {
        opts.traversal.extra_target_commit_id = meta.data().default_target.as_ref().map(|t| t.sha);
    }
    but_workspace::ref_info(existing_ref, meta, opts)
}

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
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    // We can look at a workspace ref directly (via HEAD)
    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let at = repo.find_reference("refs/heads/A")?;
    let info = ref_info(at, &meta, opts.clone())?;
    // Information doesn't change just because the starting point is different.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    // Remove remote configuration to have it deduce the remote.
    // This is from the times when GB wouldn't set up Git remotes, so it's for backward compatibility.
    repo.config_snapshot_mut()
        .remove_section("branch", info.stacks[0].name().unwrap().shorten().as_bstr());

    let at = repo.find_reference("refs/heads/A")?;
    let info = ref_info(at, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);
    Ok(())
}

#[test]
fn two_dependent_branches_rebased_with_remotes() -> anyhow::Result<()> {
    let (repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-rebased-with-remotes")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * d909178 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3ba6995 (B-on-A) change in B
    * f504e38 (A) change after push
    | * ec39463 (origin/B-on-A) change in B
    |/  
    * 807f596 (origin/A) change in A
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 0, "B-on-A", StackState::InWorkspace, &["A"]);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits: [
                            LocalCommit(3ba6995, "change in B\n", local/remote(ec39463)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "f504e38",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(f504e38, "change after push\n", local),
                            LocalCommit(807f596, "change in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommits,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
    * d909178 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3ba6995 (B-on-A) change in B
    * f504e38 (A) change after push
    | * ec39463 (origin/B-on-A) change in B
    |/  
    * 807f596 (origin/A, base-of-A) change in A
    * fafd9d0 (origin/main, main) init
    ");

    // Note how `base-of-A` is absent, it's just something the user may have added,
    // and it comes with an official remote configuration.
    add_stack_with_segments(&mut meta, 0, "B-on-A", StackState::InWorkspace, &["A"]);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits: [
                            LocalCommit(3ba6995, "change in B\n", local/remote(ec39463)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "f504e38",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(f504e38, "change after push\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "807f596",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/base-of-A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(807f596, "change in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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

    add_stack_with_segments(&mut meta, 0, "B-on-A", StackState::InWorkspace, &["A"]);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits: [
                            LocalCommit(de11c03, "change in B\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "0ee3a9e",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(0ee3a9e, "change in A\n", integrated(0ee3a9e)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: Integrated,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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

    add_stack_with_segments(&mut meta, 0, "B-on-A", StackState::InWorkspace, &["A"]);

    let opts = standard_options();
    // With the standard targets, A is considered integrated.
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(0ee3a9e12c17b59a8507bbfe2ae98ab362feb21a),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits: [
                            LocalCommit(de11c03, "change in B\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "0ee3a9e",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(4),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    // If we set a reasonably old extra target, then the A segment, despite integrated, is shown.
    meta.data_mut()
        .default_target
        .as_mut()
        .expect("target setup")
        .sha = repo.rev_parse_single("fafd9d0")?.detach();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "refs/remotes/origin/B-on-A",
                        commits: [
                            LocalCommit(de11c03, "change in B\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "0ee3a9e",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(0ee3a9e, "change in A\n", integrated(0ee3a9e)),
                        ],
                        commits_on_remote: [
                            Commit(059cc4f, "Merge branch \'B-on-A\' into new-origin-A\n"),
                        ],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: Integrated,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(3),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);
    Ok(())
}

#[test]
fn two_dependent_branches_first_rebased_and_merged_into_target() -> anyhow::Result<()> {
    let (mut repo, mut meta) =
        read_only_in_memory_scenario("two-dependent-branches-first-rebased-and-merged")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0b6b861 (origin/main, origin/A) A
    | * 4f08b8d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * da597e8 (B) B
    | * 1818c17 (A) A
    |/  
    * 281456a (main) init
    ");

    add_workspace(&mut meta);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: Some(
                    Sha1(281456a55524d78e1e0ecab946032423aec1abe8),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(da597e8, "B\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "1818c17",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(1818c17, "A\n", integrated(0b6b861)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: Integrated,
                        base: "281456a",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    repo.config_snapshot_mut()
        .remove_section("remote", Some("origin".into()));
    let info = head_info(&repo, &meta, opts)?;
    // Without remote setup, remotes can't be deducted. However, we still have a commits reachable from the target remote tracking
    // branch up to the workspace base, which we should consider.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: Some(
                    Sha1(281456a55524d78e1e0ecab946032423aec1abe8),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/B",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(da597e8, "B\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "1818c17",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(1818c17, "A\n", integrated(0b6b861)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "281456a",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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

    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let opts = standard_options();
    let info = ref_info(repo.find_reference("A")?, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d5d3a92, "unique local tip\n", local),
                            LocalCommit(6ffd040, "shared by name\n", local/remote(a9954f1)),
                            LocalCommit(4cd56ab, "unique local\n", local),
                            LocalCommit(872c22f, "shared local/remote\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(50d31c8, "unique remote\n"),
                        ],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
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
        add_stack(&mut meta, idx as u128, name, StackState::InWorkspace);
    }

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane-2",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(93d7eac, "change 2\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000002,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
        0,
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        1,
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );

    // The stacks should come out just like defined above, "lane" and then "lane2" with all the right segments.
    // The lane-segment01|02 bits are brought up as dependent branch as well.
    let opts = standard_options();
    let info = ref_info(repo.find_reference("lane")?, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(7),
                        ref_name: "refs/heads/lane-segment-01",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(8),
                        ref_name: "refs/heads/lane-segment-02",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane-2",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/lane-2-segment-01",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/lane-2-segment-02",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    // Natural order here is `lane` first, but we say we want `lane-2` first
    meta.data_mut().branches.clear();
    add_stack_with_segments(
        &mut meta,
        0,
        "lane-2",
        StackState::InWorkspace,
        &["lane-2-segment-01", "lane-2-segment-02"],
    );
    add_stack_with_segments(
        &mut meta,
        1,
        "lane",
        StackState::InWorkspace,
        &["lane-segment-01", "lane-segment-02"],
    );

    let opts = standard_options();
    let info = ref_info(repo.find_reference("lane")?, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane-2",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/lane-2-segment-01",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/lane-2-segment-02",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(7),
                        ref_name: "refs/heads/lane-segment-01",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(8),
                        ref_name: "refs/heads/lane-segment-02",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
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
        add_stack(&mut meta, idx as u128, name, StackState::InWorkspace);
    }

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
        0,
        "dependant",
        StackState::InWorkspace,
        &["advanced-lane"],
    );

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "cbc6713",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(5),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    // Put it below - this is fine, new commits will the placed onto `base`.
    add_stack_with_segments(
        &mut meta,
        0,
        "advanced-lane",
        StackState::InWorkspace,
        &["dependant"],
    );

    // Even though we *could* special-case this to keep the commit in the branch that has a remote,
    // we just keep it below at all times. The frontend currently only creates them on top, for good reason.
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "cbc6713",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(5),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
        0,
        "on-top-of-dependant",
        StackState::InWorkspace,
        &["dependant", "advanced-lane"],
    );

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/on-top-of-dependant",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "cbc6713",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(7),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(5),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    meta.data_mut().branches.clear();
    add_stack_with_segments(
        &mut meta,
        0,
        "dependant",
        StackState::InWorkspace,
        &["on-top-of-dependant", "advanced-lane"],
    );

    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/dependant",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/on-top-of-dependant",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "cbc6713",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(7),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/advanced-lane",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(5),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);
    Ok(())
}

#[test]
fn two_branches_stacked_with_remotes() -> anyhow::Result<()> {
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
        0,
        "on-top-of-lane",
        StackState::InWorkspace,
        &["lane"],
    );

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/on-top-of-lane",
                        remote_tracking_ref_name: "refs/remotes/origin/on-top-of-lane",
                        commits: [
                            LocalCommit(788ad06, "change on top\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "cbc6713",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "refs/remotes/origin/lane",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local/remote(identity)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: NothingToPush,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let opts = standard_options();
    let info = ref_info(repo.find_reference("A")?, &meta, opts).unwrap();

    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(aadad9d, "shared by name\n", local/remote(2b1808c)),
                            LocalCommit(96a2408, "another unrelated\n", integrated(96a2408)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: UnpushedCommitsRequiringForce,
                        base: "f15ca75",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/integrated",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(f15ca75, "other integrated\n", integrated(f15ca75)),
                            LocalCommit(9456d79, "integrated in target\n", integrated(9456d79)),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(3),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
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
        add_stack(&mut meta, idx as u128, name, StackState::InWorkspace);
    }

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    // It's fine to have no managed commit, but we have to deal with it - see flag is_managed.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let info = ref_info(repo.find_reference("advanced-lane")?, &meta, opts).unwrap();
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
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
        add_stack(&mut meta, idx as u128, name, StackState::InWorkspace);
    }

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    // Everything is show so the workspace stays clear, the entrypoint says what to focus on.
    let info = ref_info(repo.find_reference("advanced-lane")?, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(3),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    let info = ref_info(repo.find_reference("lane")?, &meta, opts.clone())?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(0),
        ),
        lower_bound: Some(
            NodeIndex(0),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    meta.data_mut().branches.clear();
    // Invert the order to invert stack order.
    for (idx, name) in ["advanced-lane", "lane"].into_iter().enumerate() {
        add_stack(&mut meta, idx as u128, name, StackState::InWorkspace);
    }

    let info = head_info(&repo, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000000,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/advanced-lane",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(cbc6713, "change\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/lane",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "fafd9d0",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 1,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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

    add_stack(&mut meta, 1, "disjoint", StackState::InWorkspace);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts)?;

    // We see the commit in the branch as there is no base to hide it.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/disjoint",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/disjoint",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(32791d2, "disjoint init\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
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

    add_stack(&mut meta, 1, "C-on-A", StackState::InWorkspace);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;

    // The shared "A" segment is used in both stacks, as it's reachable from both.
    // Stack A isn't listed, so it has no stack id.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
            Stack {
                id: None,
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let info = ref_info(repo.find_reference("C-on-A")?, &meta, opts.clone())?;

    // A partial workspace is provided, but the entire workspace is known.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
            Stack {
                id: None,
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    let b_info = ref_info(repo.find_reference("B-on-A")?, &meta, opts.clone())?;

    // It's like the stack is part of the workspace, the result is the same, with entrypoints changed.
    insta::assert_debug_snapshot!(b_info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
            Stack {
                id: None,
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(5),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    let a_info = ref_info(repo.find_reference("A")?, &meta, opts)?;

    // We can also show segments that are part of the stack (like homing in on them), as long as they are in a workspace.
    // It's notable how there are two entrypoints, so the UI has to assure both are visible.
    insta::assert_debug_snapshot!(a_info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/C-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(5f37dbf, "add new file in C-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
            Stack {
                id: None,
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(6),
                        ref_name: "refs/heads/B-on-A",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(4e5484a, "add new file in B-on-A\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "d79bba9",
                    },
                    👉ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "refs/remotes/origin/A",
                        commits: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_on_remote: [
                            Commit(89cc2d3, "change in A\n"),
                        ],
                        commits_outside: None,
                        metadata: "None",
                        push_status: UnpushedCommitsRequiringForce,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(3),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
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

    add_stack(&mut meta, 1, "unrelated", StackState::InWorkspace);

    let opts = standard_options();
    let info = head_info(&repo, &meta, opts.clone())?;
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
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/unrelated",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let info = ref_info(repo.find_reference("unrelated")?, &meta, opts.clone())?;
    // It can be checked out with the same effect, the parent workspace is still known.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                id: Some(
                    00000000-0000-0000-0000-000000000001,
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    👉ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/unrelated",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "c166d42",
                    },
                ],
            },
        ],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(2),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: Some(
            NodeIndex(0),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: false,
    }
    "#);

    // Change the stack to be inactive, so it's not considered to be part of the workspace.
    add_stack(&mut meta, 1, "unrelated", StackState::Inactive);

    let info = head_info(&repo, &meta, opts.clone())?;
    // Now there should be no stack, it's an empty workspace.
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [],
        target: Some(
            Target {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        extra_target: Some(
            NodeIndex(1),
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    // The unrelated reference would be its own pseudo-workspace, single-branch mode effectively.
    // It's on the base and clearly outside the workspace.
    let info = ref_info(repo.find_reference("unrelated")?, &meta, opts)?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/unrelated",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/unrelated",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(c166d42, "init-integration\n", integrated(c166d42), ►main),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: Branch,
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: Some(
            NodeIndex(2),
        ),
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);
    Ok(())
}

#[test]
fn advanced_workspace_multi_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("advanced-workspace-ref")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a7131b1 (HEAD -> gitbutler/workspace) on-top4
    * 4d3831e (intermediate-ref) on-top3
    *   468357f on-top2-merge
    |\  
    | * d3166f7 (branch-on-top) on-top-sibling
    |/  
    * 118ddbb on-top1
    *   619d548 GitButler Workspace Commit
    |\  
    | * 6fdab32 (A) A1
    * | 8a352d5 (B) B1
    |/  
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

    let opts = standard_options();
    let err = head_info(&repo, &meta, opts).unwrap_err();
    insta::assert_snapshot!(err.to_string(), @r"
    Found 5 commit(s) on top of the workspace commit.

    Run the following command in your working directory to fix this while leaving your worktree unchanged.
    Worktree changes need to be re-committed manually for now.

        git reset --soft 619d5486e6f5ae7f5e4ecc4a6ea506d6c8433410
    ");
    Ok(())
}

#[test]
fn advanced_workspace_single_stack() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("advanced-workspace-ref-and-single-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * da912a8 (HEAD -> gitbutler/workspace) on-top4
    * 198eaf8 (intermediate-ref) on-top3
    *   3147997 on-top2-merge
    |\  
    | * dd7bb9a (branch-on-top) on-top-sibling
    |/  
    * 9785229 on-top1
    * c58f157 GitButler Workspace Commit
    * 6fdab32 (A) A1
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");

    add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

    let opts = standard_options();
    let err = head_info(&repo, &meta, opts).unwrap_err();
    insta::assert_snapshot!(err.to_string(), @r"
    Found 5 commit(s) on top of the workspace commit.

    Run the following command in your working directory to fix this while leaving your worktree unchanged.
    Worktree changes need to be re-committed manually for now.

        git reset --soft c58f15768a8ba280fe773ffc4820b13ad882cd84
    ");
    Ok(())
}

mod branch_details;
mod journey;
mod legacy;

pub(crate) mod utils {
    use but_graph::{
        VirtualBranchesTomlMetadata,
        init::Options,
        virtual_branches_legacy_types::{Stack, StackBranch, Target},
    };
    use but_testsupport::gix_testtools::tempfile::TempDir;
    use gitbutler_stack::StackId;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        named_read_only_in_memory_scenario("with-remotes-and-workspace", name)
    }

    pub fn named_read_only_in_memory_scenario(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let (repo, mut meta) =
            crate::ref_info::utils::named_read_only_in_memory_scenario(script, name)?;
        let vb = meta.data_mut();
        vb.default_target = Some(Target {
            // For simplicity, we stick to the defaults.
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            // Not required
            remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
            sha: repo
                .try_find_reference("main")?
                .map(|mut r| r.peel_to_id())
                .transpose()?
                .map(|id| id.detach())
                .unwrap_or_else(|| gix::hash::Kind::Sha1.null()),
            push_remote_name: None,
        });
        Ok((repo, meta))
    }

    pub fn named_writable_scenario_with_description(
        name: &str,
    ) -> anyhow::Result<(
        TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        String,
    )> {
        named_writable_scenario_with_args_and_description(name, None::<String>)
    }

    pub fn named_writable_scenario_with_description_and_graph(
        name: &str,
        init_meta: impl FnMut(&mut VirtualBranchesTomlMetadata),
    ) -> anyhow::Result<(
        TempDir,
        but_graph::Graph,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        String,
    )> {
        named_writable_scenario_with_args_and_description_and_graph(name, None::<String>, init_meta)
    }

    pub fn named_writable_scenario_with_args_and_description(
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> anyhow::Result<(
        TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        String,
    )> {
        let (tmp, repo, mut meta) =
            crate::ref_info::utils::named_writable_scenario_with_args(name, args)?;
        let vb = meta.data_mut();
        vb.default_target = Some(Target {
            // For simplicity, we stick to the defaults.
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            // Not required
            remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
            sha: repo
                .try_find_reference("main")?
                .map(|mut r| r.peel_to_id())
                .transpose()?
                .map(|id| id.detach())
                .unwrap_or_else(|| gix::hash::Kind::Sha1.null()),
            push_remote_name: None,
        });
        let desc = std::fs::read_to_string(repo.git_dir().join("description"))?;
        Ok((tmp, repo, meta, desc))
    }

    /// Use `init_meta` to configure metadata for the graph that is also returned.
    pub fn named_writable_scenario_with_args_and_description_and_graph(
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
        mut init_meta: impl FnMut(&mut VirtualBranchesTomlMetadata),
    ) -> anyhow::Result<(
        TempDir,
        but_graph::Graph,
        gix::Repository,
        VirtualBranchesTomlMetadata,
        String,
    )> {
        let (tmp, repo, mut meta, desc) =
            named_writable_scenario_with_args_and_description(name, args)?;

        init_meta(&mut meta);
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            Options {
                extra_target_commit_id: repo.rev_parse_single("main").ok().map(|id| id.detach()),
                ..Options::limited()
            },
        )?;
        Ok((tmp, graph, repo, meta, desc))
    }

    pub fn named_writable_scenario(
        name: &str,
    ) -> anyhow::Result<(TempDir, gix::Repository, VirtualBranchesTomlMetadata)> {
        let (a, b, c, _desc) = named_writable_scenario_with_description(name)?;
        Ok((a, b, c))
    }

    pub fn named_read_only_in_memory_scenario_with_description(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        String,
    )> {
        let (repo, meta) = named_read_only_in_memory_scenario(script, name)?;
        let desc = std::fs::read_to_string(repo.git_dir().join("description"))?;
        Ok((repo, meta, desc))
    }

    pub enum StackState {
        InWorkspace,
        Inactive,
    }

    pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
        add_stack(
            meta,
            u128::MAX,
            "definitely outside of the workspace just to have it",
            StackState::Inactive,
        );
    }

    pub fn add_stack(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: u128,
        stack_name: &str,
        state: StackState,
    ) -> StackId {
        add_stack_with_segments(meta, stack_id, stack_name, state, &[])
    }

    // Add parameters as needed.
    pub fn add_stack_with_segments(
        meta: &mut VirtualBranchesTomlMetadata,
        stack_id: u128,
        stack_name: &str,
        state: StackState,
        segments: &[&str],
    ) -> StackId {
        let mut stack = Stack::new_with_just_heads(
            segments
                .iter()
                .rev()
                .map(|stack_name| {
                    StackBranch::new_with_zero_head((*stack_name).into(), None, None, None, false)
                })
                .chain(std::iter::once(StackBranch::new_with_zero_head(
                    stack_name.into(),
                    None,
                    None,
                    None,
                    false,
                )))
                .collect(),
            0,
            meta.data().branches.len(),
            match state {
                StackState::InWorkspace => true,
                StackState::Inactive => false,
            },
        );
        stack.order = stack_id as usize;
        let stack_id = StackId::from_number_for_testing(stack_id);
        stack.id = stack_id;
        meta.data_mut().branches.insert(stack_id, stack);
        stack_id
    }
}
use utils::add_stack;
pub use utils::read_only_in_memory_scenario;

use crate::ref_info::{
    utils::standard_options,
    with_workspace_commit::utils::{StackState, add_stack_with_segments, add_workspace},
};
