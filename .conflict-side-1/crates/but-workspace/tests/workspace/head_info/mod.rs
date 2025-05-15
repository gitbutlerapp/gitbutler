use crate::head_info::utils::read_only_in_memory_scenario;
use but_workspace::head_info;

/// All tests that use a workspace commit for a fully managed, explicit workspace.
mod with_workspace_commit;

#[test]
fn untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 5,
            expensive_commit_info: true,
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
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [],
                        commits_unique_in_remote_tracking_branch: [],
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
    let info = but_workspace::head_info(&repo, &*meta, head_info::Options::default())?;
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

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit,
            expensive_commit_info: true,
        },
    )?;

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
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(b5743a3, "10\n", local),
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                            LocalCommit(c4f2a35, "6\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
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
            expensive_commit_info: true,
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
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(b5743a3, "10\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: "refs/heads/nine",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: "refs/heads/six",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(c4f2a35, "6\n", local),
                            LocalCommit(44c12ce, "5\n", local),
                            LocalCommit(c584dbe, "4\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: "refs/heads/three",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(281da94, "3\n", local),
                            LocalCommit(12995d7, "2\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: "refs/heads/one",
                        remote_tracking_ref_name: "None",
                        ref_location: "AtHead",
                        commits_unique_from_tip: [
                            LocalCommit(3d57fc1, "1\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
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
}
