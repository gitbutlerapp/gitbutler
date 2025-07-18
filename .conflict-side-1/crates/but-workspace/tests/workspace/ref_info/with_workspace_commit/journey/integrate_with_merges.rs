//! A loose collection of states that users typically encounter.
use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::utils::named_read_only_in_memory_scenario_with_description;
use but_graph::VirtualBranchesTomlMetadata;
use but_testsupport::visualize_commit_graph_all;

#[test]
fn two_commits_require_force_push() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-one-rewritten-one-local-after-push")?;
    insta::assert_snapshot!(description, @r"
    A setup that demands for a force-push

    We change the name of the first commit and also need the similarity to be detected by changeset
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0d9835f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a1b4326 (A) A2
    * e1f216e A1
    | * 3fcd07a (origin/A) A1 (same but different)
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a1b4326, "A2\n", local),
                                LocalCommit(e1f216e, "A1\n", local/remote(similarity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: UnpushedCommitsRequiringForce,
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
            extra_target: None,
            lower_bound: Some(
                NodeIndex(2),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_diverged() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("02-diverged-remote")?;
    insta::assert_snapshot!(description, @r"
    A setup that demands for a force-push

    The tip of the local branch isn't in the ancestry of the remote anymore.
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a62b0de (A) A2
    | * 0c06863 (origin/A) A3
    |/  
    * 120a217 A1
    * fafd9d0 (origin/main, main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [
                                Commit(0c06863, "A3\n"),
                            ],
                            metadata: "None",
                            push_status: UnpushedCommitsRequiringForce,
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
            extra_target: None,
            lower_bound: Some(
                NodeIndex(2),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_behind() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("03-remote-one-behind")?;
    insta::assert_snapshot!(description, @"A can be pushed as it has local, unpushed commits");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a62b0de (A) A2
    * 120a217 (origin/A) A1
    * fafd9d0 (origin/main, main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
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
            extra_target: None,
            lower_bound: Some(
                NodeIndex(2),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_ahead() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("04-remote-one-ahead-ff")?;
    insta::assert_snapshot!(description, @"There are no unpushed local commits, the remote is one ahead (FF)");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8ee08de (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * a62b0de (origin/A) A2
    |/  
    * 120a217 (A) A1
    * fafd9d0 (origin/main, main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [
                                Commit(a62b0de, "A2\n"),
                            ],
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
            extra_target: None,
            lower_bound: Some(
                NodeIndex(2),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

pub fn scenario(
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    String,
)> {
    named_read_only_in_memory_scenario_with_description("journey02", name)
}
