use but_testsupport::visualize_commit_graph_all;

use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::journey::utils::standard_options_with_extra_target;
use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack_with_segments};
use utils::step;

#[test]
fn j01_unborn() -> anyhow::Result<()> {
    let (repo, meta, description) = step("01-unborn")?;
    insta::assert_snapshot!(description, @"a newly initialized repository");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
        RefInfo {
            workspace_ref_name: Some(
                FullName(
                    "refs/heads/main",
                ),
            ),
            stacks: [
                Stack {
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: None,
            is_managed_ref: false,
            is_managed_commit: false,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j02_unborn() -> anyhow::Result<()> {
    let (repo, meta, description) = step("02-first-commit")?;
    insta::assert_snapshot!(description, @"the root commit is now present locally");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main) init");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
        RefInfo {
            workspace_ref_name: Some(
                FullName(
                    "refs/heads/main",
                ),
            ),
            stacks: [
                Stack {
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: None,
            is_managed_ref: false,
            is_managed_commit: false,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j03_main_pushed() -> anyhow::Result<()> {
    let (repo, meta, description) = step("03-main-pushed")?;
    insta::assert_snapshot!(description, @r"
    main was pushed so it can now serve as target.

    However, without an official workspace it still won't be acting as a target.
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* fafd9d0 (HEAD -> main, origin/main) init");

    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
        RefInfo {
            workspace_ref_name: Some(
                FullName(
                    "refs/heads/main",
                ),
            ),
            stacks: [
                Stack {
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: None,
            is_managed_ref: false,
            is_managed_commit: false,
            is_entrypoint: true,
        },
    )
    "#);

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "origin/main"),
    );
    // TODO: it should also contain the extra target (but it can't express target refs as these are hashes only)
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
        RefInfo {
            workspace_ref_name: Some(
                FullName(
                    "refs/heads/main",
                ),
            ),
            stacks: [
                Stack {
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: 0,
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: None,
            is_managed_ref: false,
            is_managed_commit: false,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j04_workspace() -> anyhow::Result<()> {
    let (repo, meta, description) = step("04-create-workspace")?;
    insta::assert_snapshot!(description, @"An official workspace was created, with nothing in it");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, main) init
    ");

    // Adding an empty workspace doesn't change the outcome, this is fully graph based
    // (despite the target being set by the test-suite).
    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    insta::assert_debug_snapshot!(info, @r#"
    Ok(
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
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j05_workspace() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("05-empty-stack")?;
    insta::assert_snapshot!(description, @"an empty stack with nothing in it");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * fafd9d0 (origin/main, main, S1) init
    ");

    // We need to advertise empty stacks (i.e. independent branches) as they are not discoverable otherwise.
    // This would be configured by the function that creates the empty stack,
    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "None",
                            commits: [],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
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
fn j06_workspace() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("06-create-commit-in-stack")?;
    insta::assert_snapshot!(description, @"Create a new commit in the newly added stack S1");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9a8283b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * ba16348 (S1) one
    * fafd9d0 (origin/main, main) init
    ");

    // Now that there is a commit, the stack is picked up automatically, but without additional data.
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(ba16348, "one\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);

    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(ba16348, "one\n", local),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                            push_status: CompletelyUnpushed,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
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
fn j07_push_commit() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("07-push-commit")?;
    insta::assert_snapshot!(description, @"push S1 to the remote which is then up-to-date");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9a8283b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * ba16348 (origin/S1, S1) one
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(ba16348, "one\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                            push_status: NothingToPush,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
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
fn j08_next_local_commit() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("08-new-local-commit")?;
    insta::assert_snapshot!(description, @r"
    Create a new local commit right after the previous pushed one

      This leaves the stack in a state where it can be pushed.
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9e1f264 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9f4d478 (S1) two
    * ba16348 (origin/S1) one
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(9f4d478, "two\n", local),
                                LocalCommit(ba16348, "one\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                            push_status: UnpushedCommits,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
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
fn j09_rewritten_remote_and_local_commit() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("09-rewritten-local-commit")?;
    insta::assert_snapshot!(description, @"The new local commit was rewritten after pushing it to the remote");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8010a9f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * c591296 (S1) two
    | * c5aaf6b (origin/S1) two
    |/  
    * ba16348 one
    * fafd9d0 (origin/main, main) init
    ");

    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
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
                            id: 3,
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(c591296, "two\n", local/remote(similarity)),
                                LocalCommit(ba16348, "one\n", local/remote(identity)),
                            ],
                            commits_unique_in_remote_tracking_branch: [],
                            metadata: Branch {
                                ref_info: RefInfo { created_at: None, updated_at: "1970-01-01 00:00:00 +0000" },
                                description: None,
                                review: Review { pull_request: None, review_id: None },
                            },
                            push_status: UnpushedCommitsRequiringForce,
                        },
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/main",
                ),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

mod utils {
    use crate::ref_info::utils::standard_options;
    use crate::ref_info::with_workspace_commit::named_read_only_in_memory_scenario;
    use but_graph::VirtualBranchesTomlMetadata;

    pub fn step(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
        String,
    )> {
        let (repo, meta) = named_read_only_in_memory_scenario("journey", name)?;
        let desc = std::fs::read_to_string(repo.git_dir().join("description"))?;
        Ok((repo, meta, desc))
    }

    pub fn standard_options_with_extra_target(
        repo: &gix::Repository,
        short_name: &str,
    ) -> but_workspace::ref_info::Options {
        but_workspace::ref_info::Options {
            traversal: but_graph::init::Options {
                extra_target_commit_id: repo
                    .find_reference(short_name)
                    .expect("target reference is valid")
                    .peel_to_id_in_place()
                    .unwrap()
                    .detach()
                    .into(),
                ..Default::default()
            },
            ..standard_options()
        }
    }
}
