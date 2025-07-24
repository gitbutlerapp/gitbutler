use crate::ref_info::utils::{read_only_in_memory_scenario, standard_options};
use but_workspace::ref_info;

/// All tests that use a workspace commit for a fully managed, explicit workspace.
mod with_workspace_commit;

#[test]
fn unborn_untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = but_workspace::head_info(&repo, &*meta, standard_options())?;
    // It's clear that this branch is unborn as there is not a single commit,
    // in absence of a target ref.
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/main",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        commits: [],
                        commits_unique_in_remote_tracking_branch: [],
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
        is_entrypoint: true,
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
        workspace_ref_name: None,
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
                        commits_unique_in_remote_tracking_branch: [],
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
        is_entrypoint: true,
    }
    "#);
    Ok(())
}

#[test]
fn conflicted_in_local_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("with-conflict")?;
    let info = but_workspace::head_info(&repo, &*meta, ref_info::Options::default())?;
    // The conflict is detected in the local commit.
    insta::assert_debug_snapshot!(&info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/main",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(💥8450331, "GitButler WIP Commit\n\n\n", local),
                            LocalCommit(a047f81, "init\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
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
        is_entrypoint: true,
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
        workspace_ref_name: Some(
            FullName(
                "refs/heads/main",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/main",
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
                        commits_unique_in_remote_tracking_branch: [],
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
        is_entrypoint: true,
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
        workspace_ref_name: Some(
            FullName(
                "refs/heads/main",
            ),
        ),
        stacks: [
            Stack {
                id: None,
                base: None,
                segments: [
                    ref_info::ui::Segment {
                        id: NodeIndex(0),
                        ref_name: "refs/heads/main",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(b5743a3, "10\n", local, ►above-10),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "344e320",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(1),
                        ref_name: "refs/heads/nine",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(344e320, "9\n", local),
                            LocalCommit(599c271, "8\n", local),
                            LocalCommit(05f069b, "7\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "c4f2a35",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(2),
                        ref_name: "refs/heads/six",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(c4f2a35, "6\n", local),
                            LocalCommit(44c12ce, "5\n", local),
                            LocalCommit(c584dbe, "4\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "281da94",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(3),
                        ref_name: "refs/heads/three",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(281da94, "3\n", local),
                            LocalCommit(12995d7, "2\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
                        metadata: "None",
                        push_status: CompletelyUnpushed,
                        base: "3d57fc1",
                    },
                    ref_info::ui::Segment {
                        id: NodeIndex(4),
                        ref_name: "refs/heads/one",
                        remote_tracking_ref_name: "None",
                        commits: [
                            LocalCommit(3d57fc1, "1\n", local),
                        ],
                        commits_unique_in_remote_tracking_branch: [],
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
        is_entrypoint: true,
    }
    "#);

    assert_eq!(info.stacks[0].segments.len(), 5, "multiple segments");
    Ok(())
}

mod utils {
    use but_graph::VirtualBranchesTomlMetadata;
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

    pub fn standard_options() -> but_workspace::ref_info::Options {
        ref_info::Options {
            stack_commit_limit: 0,
            expensive_commit_info: true,
            traversal: Default::default(),
        }
    }
}
