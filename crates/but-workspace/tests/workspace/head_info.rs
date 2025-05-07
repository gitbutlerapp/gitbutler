use crate::head_info::utils::read_only_in_memory_scenario;
use but_workspace::head_info;

#[test]
fn untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 5,
        },
    )?;
    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: None,
                base: None,
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);
    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("one-commit-detached")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 0,
        },
    )?;
    insta::assert_debug_snapshot!(&info, @r"
    HeadInfo {
        stacks: [],
        target_ref: None,
    }
    ");
    Ok(())
}

#[test]
fn single_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let stack_commit_limit = 5;

    let info = but_workspace::head_info(&repo, &*meta, head_info::Options { stack_commit_limit })?;

    assert_eq!(
        info.stacks[0].segments.len(),
        1,
        "a single branch, a single segment"
    );
    assert_eq!(
        info.stacks[0].segments[0].commits_unique_from_tip.len(),
        stack_commit_limit,
        "commit limit is respected"
    );
    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: Some(
                    Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                ),
                base: None,
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(b5743a3, "10\n", local),
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                            LocalCommit(c4f2a35, "6\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);
    Ok(())
}

#[test]
fn single_branch_multiple_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits-multi-segment")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 0,
        },
    )?;

    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: Some(
                    Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                ),
                base: None,
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(b5743a3, "10\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/nine",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/six",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(c4f2a35, "6\n", local),
                            LocalCommit(44c12ce, "5\n", local),
                            LocalCommit(c584dbe, "4\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/three",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(281da94, "3\n", local),
                            LocalCommit(12995d7, "2\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/one",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            LocalCommit(3d57fc1, "1\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);

    assert_eq!(info.stacks[0].segments.len(), 5, "multiple segments");
    Ok(())
}

mod utils {
    use but_workspace::VirtualBranchesTomlMetadata;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = crate::utils::read_only_in_memory_scenario(name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }
}
