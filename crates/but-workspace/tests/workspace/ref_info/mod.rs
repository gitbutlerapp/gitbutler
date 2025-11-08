use but_workspace::{StacksFilter, ref_info, stack_details_v3, stacks_v3};

use crate::ref_info::utils::{read_only_in_memory_scenario, standard_options};

/// All tests that use a workspace commit for a fully managed, explicit workspace.
pub(crate) mod with_workspace_commit;

#[test]
fn unborn_untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = but_workspace::head_info(&repo, &*meta, standard_options())?;
    // It's clear that this branch is unborn as there is not a single commit,
    // in absence of a target ref.
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/main",
                ),
                worktree: Some(
                    Main,
                ),
            },
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "►main[🌳]",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: None,
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    // It's now possible to use the old API with unborn repos.
    // This type can't really represent missing tips, but `null()` will do.
    insta::assert_debug_snapshot!(&stacks, @r#"
    [
        StackEntry {
            id: None,
            heads: [
                StackHeadInfo {
                    name: "main",
                    tip: Sha1(0000000000000000000000000000000000000000),
                    is_checked_out: false,
                },
            ],
            tip: Sha1(0000000000000000000000000000000000000000),
            order: None,
            is_checked_out: false,
        },
    ]
    "#);

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // It's also possible to obtain details.
    insta::assert_debug_snapshot!(&details, @r#"
    StackDetails {
        derived_name: "main",
        push_status: CompletelyUnpushed,
        branch_details: [
            BranchDetails {
                name: "main",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(0000000000000000000000000000000000000000),
                base_commit: Sha1(0000000000000000000000000000000000000000),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [],
                is_conflicted: false,
                commits: [],
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
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("one-commit-detached")?;
    let info = but_workspace::head_info(&repo, &*meta, ref_info::Options::default())?;
    // As the workspace name is derived from the first segment, it's empty as well.
    // We do know that `main` is pointing at the local commit though, despite the unnamed segment owning it.
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_info: None,
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "None",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(15bcd1b, "init\n", local, ►main),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: None,
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    // Detached heads can't be represented with this API as it really needs a name.
    insta::assert_debug_snapshot!(&stacks, @"[]");

    let err = stack_details_v3(None, &repo, &meta).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Can't handle a stack yet whose tip isn't pointed to by a ref"
    );
    Ok(())
}

#[test]
fn conflicted_in_local_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("with-conflict")?;
    let info = but_workspace::head_info(&repo, &*meta, ref_info::Options::default())?;
    // The conflict is detected in the local commit.
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/main",
                ),
                worktree: Some(
                    Main,
                ),
            },
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "►main[🌳]",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(💥8450331, "GitButler WIP Commit\n\n\n", local),
                            LocalCommit(a047f81, "init\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: None,
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    insta::assert_debug_snapshot!(&stacks, @r#"
    [
        StackEntry {
            id: None,
            heads: [
                StackHeadInfo {
                    name: "main",
                    tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
                    is_checked_out: false,
                },
            ],
            tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
            order: None,
            is_checked_out: false,
        },
    ]
    "#);

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // The conflict is visible here as well.
    insta::assert_debug_snapshot!(details, @r#"
    StackDetails {
        derived_name: "main",
        push_status: CompletelyUnpushed,
        branch_details: [
            BranchDetails {
                name: "main",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(84503317a1e1464381fcff65ece14bc1f4315b7c),
                base_commit: Sha1(0000000000000000000000000000000000000000),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    GitButler <gitbutler@gitbutler.com>,
                    author <author@example.com>,
                ],
                is_conflicted: true,
                commits: [
                    Commit(8450331, "GitButler WIP Commit", local),
                    Commit(a047f81, "init", local),
                ],
                upstream_commits: [],
                is_remote_head: false,
            },
        ],
        is_conflicted: true,
    }
    "#);
    Ok(())
}

#[test]
fn single_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let info = but_workspace::head_info(&repo, &*meta, standard_options())?;

    assert_eq!(
        info.stacks[0].segments.len(),
        1,
        "a single branch, a single segment"
    );
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/main",
                ),
                worktree: Some(
                    Main,
                ),
            },
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "►main[🌳]",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(b5743a3, "10\n", local),
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                            LocalCommit(c4f2a35, "6\n", local),
                            LocalCommit(44c12ce, "5\n", local),
                            LocalCommit(c584dbe, "4\n", local),
                            LocalCommit(281da94, "3\n", local),
                            LocalCommit(12995d7, "2\n", local),
                            LocalCommit(3d57fc1, "1\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: None,
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    insta::assert_debug_snapshot!(&stacks, @r#"
    [
        StackEntry {
            id: None,
            heads: [
                StackHeadInfo {
                    name: "main",
                    tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                    is_checked_out: false,
                },
            ],
            tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            order: None,
            is_checked_out: false,
        },
    ]
    "#);

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    insta::assert_debug_snapshot!(details, @r#"
    StackDetails {
        derived_name: "main",
        push_status: CompletelyUnpushed,
        branch_details: [
            BranchDetails {
                name: "main",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                base_commit: Sha1(0000000000000000000000000000000000000000),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(b5743a3, "10", local),
                    Commit(344e320, "9", local),
                    Commit(599c271, "8", local),
                    Commit(05f069b, "7", local),
                    Commit(c4f2a35, "6", local),
                    Commit(44c12ce, "5", local),
                    Commit(c584dbe, "4", local),
                    Commit(281da94, "3", local),
                    Commit(12995d7, "2", local),
                    Commit(3d57fc1, "1", local),
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
fn single_branch_multiple_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits-multi-segment")?;
    let info = but_workspace::head_info(&repo, &*meta, standard_options())?;

    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/main",
                ),
                worktree: Some(
                    Main,
                ),
            },
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "►main[🌳]",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(b5743a3, "10\n", local, ►above-10),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "344e320",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(1),
                        ref_name: "►nine",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "c4f2a35",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(2),
                        ref_name: "►six",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(c4f2a35, "6\n", local),
                            LocalCommit(44c12ce, "5\n", local),
                            LocalCommit(c584dbe, "4\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "281da94",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "►three",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(281da94, "3\n", local),
                            LocalCommit(12995d7, "2\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "3d57fc1",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "►one",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(3d57fc1, "1\n", local),
                        ],
                        commits_on_remote: [],
                        commits_outside: None,
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "None",
                    },
                ],
            },
        ],
        target: None,
        extra_target: None,
        lower_bound: None,
        is_managed_ref: false,
        is_managed_commit: false,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    }
    "#);

    assert_eq!(info.stacks[0].segments.len(), 5, "multiple segments");

    let stacks = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
    insta::assert_debug_snapshot!(&stacks, @r#"
    [
        StackEntry {
            id: None,
            heads: [
                StackHeadInfo {
                    name: "main",
                    tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                    is_checked_out: false,
                },
                StackHeadInfo {
                    name: "nine",
                    tip: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                    is_checked_out: false,
                },
                StackHeadInfo {
                    name: "six",
                    tip: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                    is_checked_out: false,
                },
                StackHeadInfo {
                    name: "three",
                    tip: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
                    is_checked_out: false,
                },
                StackHeadInfo {
                    name: "one",
                    tip: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
                    is_checked_out: false,
                },
            ],
            tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
            order: None,
            is_checked_out: false,
        },
    ]
    "#);

    let details = stack_details_v3(stacks[0].id, &repo, &meta)?;
    // It also works with multiple segments.
    insta::assert_debug_snapshot!(details, @r#"
    StackDetails {
        derived_name: "main",
        push_status: CompletelyUnpushed,
        branch_details: [
            BranchDetails {
                name: "main",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                base_commit: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(b5743a3, "10", local),
                ],
                upstream_commits: [],
                is_remote_head: false,
            },
            BranchDetails {
                name: "nine",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                base_commit: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(344e320, "9", local),
                    Commit(599c271, "8", local),
                    Commit(05f069b, "7", local),
                ],
                upstream_commits: [],
                is_remote_head: false,
            },
            BranchDetails {
                name: "six",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                base_commit: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(c4f2a35, "6", local),
                    Commit(44c12ce, "5", local),
                    Commit(c584dbe, "4", local),
                ],
                upstream_commits: [],
                is_remote_head: false,
            },
            BranchDetails {
                name: "three",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
                base_commit: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(281da94, "3", local),
                    Commit(12995d7, "2", local),
                ],
                upstream_commits: [],
                is_remote_head: false,
            },
            BranchDetails {
                name: "one",
                remote_tracking_branch: None,
                description: None,
                pr_number: None,
                review_id: None,
                tip: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
                base_commit: Sha1(0000000000000000000000000000000000000000),
                push_status: CompletelyUnpushed,
                last_updated_at: None,
                authors: [
                    author <author@example.com>,
                ],
                is_conflicted: false,
                commits: [
                    Commit(3d57fc1, "1", local),
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

mod utils {
    use but_graph::VirtualBranchesTomlMetadata;
    use but_testsupport::gix_testtools::tempfile;
    use but_workspace::ref_info;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        named_read_only_in_memory_scenario(name, "")
    }

    pub fn named_read_only_in_memory_scenario(
        script: &str,
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = crate::utils::read_only_in_memory_scenario_named(script, name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }

    pub fn named_writable_scenario_with_args(
        name: &str,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        VirtualBranchesTomlMetadata,
    )> {
        let (repo, tmp) = crate::utils::writable_scenario_with_args(name, args);
        let meta =
            VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))?;
        Ok((tmp, repo, meta))
    }

    pub fn standard_options() -> but_workspace::ref_info::Options {
        ref_info::Options {
            expensive_commit_info: true,
            traversal: Default::default(),
        }
    }
}
