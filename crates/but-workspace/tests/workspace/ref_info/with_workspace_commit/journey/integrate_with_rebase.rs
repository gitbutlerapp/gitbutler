//! A loose collection of states that users typically encounter.
use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::utils::named_read_only_in_memory_scenario_with_description;
use but_graph::VirtualBranchesTomlMetadata;
use but_testsupport::visualize_commit_graph_all;

#[test]
fn two_commits_rebased_onto_target() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-one-rewritten-one-local-after-push")?;
    insta::assert_snapshot!(description, @r"
    two local commits pushed to a remote, then rebased onto target.

    The branch should then be considered integrated
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 946cdb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f9c2b14 (origin/A, A) A2
    * e1f216e A1
    | * eabf298 (origin/main, main) M3
    | * 1ff86ae M2
    | * 36c87e5 A2
    | * 818dbb2 A1
    | * ce09734 M1
    |/  
    * fafd9d0 init
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
                                LocalCommit(f9c2b14, "A2\n", integrated(36c87e5)),
                                LocalCommit(e1f216e, "A1\n", integrated(818dbb2)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
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
                    commits_ahead: 5,
                },
            ),
            extra_target: None,
            lower_bound: Some(
                NodeIndex(5),
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
fn two_commits_rebased_onto_target_with_changeset_check() -> anyhow::Result<()> {
    let (repo, meta, description) =
        scenario("01-one-rewritten-one-local-after-push-author-date-change")?;
    insta::assert_snapshot!(description, @r"
    two local commits pushed to a remote, then rebased onto target, but with the author date adjusted.

    This prevents quick-checks to work.
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f1caa51 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f9c2b14 (origin/A, A) A2
    * e1f216e A1
    | * 7a2d071 (origin/main, main) M3
    | * 34dac9d M2
    | * 85063c1 A2
    | * 444639d A1
    | * ce09734 M1
    |/  
    * fafd9d0 init
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
                                LocalCommit(f9c2b14, "A2\n", integrated(85063c1)),
                                LocalCommit(e1f216e, "A1\n", integrated(444639d)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
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
                    commits_ahead: 5,
                },
            ),
            extra_target: None,
            lower_bound: Some(
                NodeIndex(5),
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
    named_read_only_in_memory_scenario_with_description("journey03", name)
}
