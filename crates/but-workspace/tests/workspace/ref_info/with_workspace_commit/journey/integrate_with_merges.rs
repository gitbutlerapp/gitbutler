//! A loose collection of states that users typically encounter.
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::visualize_commit_graph_all;

use crate::ref_info::{
    utils::standard_options,
    with_workspace_commit::{
        journey::utils::standard_options_with_extra_target,
        utils::named_read_only_in_memory_scenario_with_description,
    },
};

#[test]
fn two_commits_require_force_push() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-one-rewritten-one-local-after-push")?;
    insta::assert_snapshot!(description, @r"
    A setup that demands for a force-push

    We change the name of the first commit and also need the similarity to be detected by changeset
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 946cdb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f9c2b14 (A) A2
    * e1f216e A1
    | * 3fcd07a (origin/A) A1 (same but different)
    |/  
    * fafd9d0 (origin/main, main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(f9c2b14, "A2\n", local),
                                LocalCommit(e1f216e, "A1\n", local/remote(3fcd07a)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
            ancestor_workspace_commit: None,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn two_commits_require_force_push_merged() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01.5-one-rewritten-one-local-after-push-merge")?;
    insta::assert_snapshot!(description, @"On the remote, a rewritten/rebased commit we have locally is merged back into target.");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 946cdb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f9c2b14 (A) A2
    * e1f216e A1
    | * c635f08 (origin/main) merge origin/A
    |/| 
    | * 3fcd07a (origin/A) A1 (same but different)
    |/  
    * fafd9d0 (main) init
    ");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(f9c2b14, "A2\n", local),
                                LocalCommit(e1f216e, "A1\n", integrated(c635f08)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
                    commits_ahead: 2,
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_on_remote: [
                                Commit(0c06863, "A3\n"),
                            ],
                            commits_outside: None,
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
            ancestor_workspace_commit: None,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_diverged_merge() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("02.5-diverged-remote-merge")?;
    insta::assert_snapshot!(description, @r"
    A remote sharing a commit with a stack and its own commit gets merged.

    We'd not want to see the remote unique commit anymore as it's also considered integrated.
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a62b0de (A) A2
    | *   085089c (origin/main, main) Merge remote-tracking branch 'origin/A'
    | |\  
    | | * 0c06863 (origin/A) A3
    | |/  
    |/|   
    * | 120a217 A1
    |/  
    * fafd9d0 init
    ");

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "fafd9d0"),
    );
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(5),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", integrated(120a217)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
                    commits_ahead: 2,
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
            ancestor_workspace_commit: None,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_behind_merge_no_ff() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("03.5-remote-one-behind-merge-no-ff")?;
    insta::assert_snapshot!(description, @"Remote origin/A is merged back (with forceful merge commit) while there are still local commits.");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3ea2742 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * a62b0de (A) A2
    | *   a670cd5 (origin/main, main) Merge remote-tracking branch 'origin/A'
    | |\  
    | |/  
    |/|   
    * | 120a217 (origin/A) A1
    |/  
    * fafd9d0 init
    ");

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "fafd9d0"),
    );
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(5),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(a62b0de, "A2\n", local),
                                LocalCommit(120a217, "A1\n", integrated(120a217)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(120a217, "A1\n", local/remote(identity)),
                            ],
                            commits_on_remote: [
                                Commit(a62b0de, "A2\n"),
                            ],
                            commits_outside: None,
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
            ancestor_workspace_commit: None,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn remote_ahead_merge_ff() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("04.5-remote-one-ahead-ff-merge")?;
    insta::assert_snapshot!(description, @"Remote origin/A is merged back (fast-forward), bringing all into the target branch");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8ee08de (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * a62b0de (origin/main, origin/A, main) A2
    |/  
    * 120a217 (A) A1
    * fafd9d0 init
    ");

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "fafd9d0"),
    );
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            stacks: [
                Stack {
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(4),
                            ref_name: "►A",
                            remote_tracking_ref_name: "refs/remotes/origin/A",
                            commits: [
                                LocalCommit(120a217, "A1\n", integrated(120a217)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
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
