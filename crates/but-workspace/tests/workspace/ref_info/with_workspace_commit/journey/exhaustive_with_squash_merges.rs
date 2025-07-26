//! A collection of tests that build on top of each other, like a progression of steps a user could take.
use but_graph::VirtualBranchesTomlMetadata;
use but_testsupport::visualize_commit_graph_all;

use crate::ref_info::utils::standard_options;
use crate::ref_info::with_workspace_commit::journey::utils::standard_options_with_extra_target;
use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_read_only_in_memory_scenario_with_description,
};

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
                    id: None,
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(0),
                            ref_name: "refs/heads/main",
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
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j02_first_commit() -> anyhow::Result<()> {
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
                    id: None,
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(0),
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
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
                    id: None,
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(0),
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", local),
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
            is_entrypoint: true,
        },
    )
    "#);

    let info = but_workspace::head_info(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "origin/main"),
    );
    // With an extra target, even in this situation we have a notion of upstream commits.
    // Thus it's possible to compute the integation status.
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
                    id: None,
                    base: None,
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(0),
                            ref_name: "refs/heads/main",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(fafd9d0, "init\n", integrated(fafd9d0)),
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
            extra_target: Some(
                NodeIndex(0),
            ),
            lower_bound: None,
            is_managed_ref: false,
            is_managed_commit: false,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

#[test]
fn j04_create_workspace() -> anyhow::Result<()> {
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
fn j05_empty_stack() -> anyhow::Result<()> {
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
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
fn j06_create_commit_in_stack() -> anyhow::Result<()> {
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
                    id: None,
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(ba16348, "one\n", local),
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(ba16348, "one\n", local),
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(ba16348, "one\n", local/remote(identity)),
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(9f4d478, "two\n", local),
                                LocalCommit(ba16348, "one\n", local/remote(identity)),
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
fn j09_rewritten_remote_and_local_commit() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("09-rewritten-local-commit")?;
    insta::assert_snapshot!(description, @"The new local commit was rewritten after pushing it to the remote");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4d23090 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 314cacb (S1) two
    | * 9a2fcdf (origin/S1) two
    |/  
    * 3234835 one
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(314cacb, "two\n", local/remote(9a2fcdf)),
                                LocalCommit(3234835, "one\n", local/remote(identity)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                            metadata: Branch,
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
fn j10_squash_merge_stack() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("10-squash-merge-stack")?;
    insta::assert_snapshot!(description, @r"
The remote squash-merges S1 *and* changes the 'file' so it looks entirely different in another commit.

  The squash merge should still be detected.
");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4d23090 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 314cacb (S1) two
    | * 9a2fcdf (origin/S1) two
    |/  
    * 3234835 one
    | * adc9f0c (origin/main) file changed completely afterwards
    | * d110262 squash S1
    |/  
    * fafd9d0 (main) init
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(314cacb, "two\n", integrated(d110262)),
                                LocalCommit(3234835, "one\n", integrated(d110262)),
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
                    commits_ahead: 2,
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
fn j11_squash_merge_remote_only() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("11-remote-only")?;
    insta::assert_snapshot!(description, @r"
The remote was re-used and merged once more with more changes.

  After S1 was squash-merged, someone else re-used the branch, pushed two commits
  and squash-merged them into target again.

  Here we assure that these integrated remote commits don't mess with our logic.
");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4d23090 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 314cacb (S1) two
    | * 16d0628 (origin/S1) add other remote file
    | * 66fe1d7 add remote file
    | * 9a2fcdf two
    |/  
    * 3234835 one
    | * 8965172 (origin/main) other remote file changed completely afterwards
    | * 9096eba squash origin/S1
    | * ccaef5a avoid merge conflcit
    | * adc9f0c (main) file changed completely afterwards
    | * d110262 squash S1
    |/  
    * fafd9d0 init
    ");

    add_stack_with_segments(&mut meta, 0, "S1", StackState::InWorkspace, &[]);
    let info = but_workspace::head_info(&repo, &*meta, standard_options());
    // TODO: remote-only squashes aren't currently detected (so remote commits are visible),
    //       but could be if it was common.
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(314cacb, "two\n", integrated(d110262)),
                                LocalCommit(3234835, "one\n", integrated(d110262)),
                            ],
                            commits_on_remote: [
                                Commit(16d0628, "add other remote file\n"),
                                Commit(66fe1d7, "add remote file\n"),
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
fn j12_local_only_multi_segment_squash_merge() -> anyhow::Result<()> {
    let (repo, mut meta, description) = step("12-local-only-multi-segment-squash-merge")?;
    insta::assert_snapshot!(description, @r"
A new multi-segment stack is created without remote and squash merged locally.

  There is no need to add the local branches to the workspace officially, they are still picked up.
  This allows the user to manually manipulate the workspace and it will work just the same.
");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   4da5b24 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 1af5d57 (local) new local file
    | * de02b20 (local-bottom) new local-bottom file
    * | 314cacb (S1) two
    | | * 16d0628 (origin/S1) add other remote file
    | | * 66fe1d7 add remote file
    | | * 9a2fcdf two
    | |/  
    |/|   
    * | 3234835 one
    |/  
    | * 639afcf (origin/main) local file rewritten completely
    | * ca783a4 squash local
    | * 8965172 (main) other remote file changed completely afterwards
    | * 9096eba squash origin/S1
    | * ccaef5a avoid merge conflcit
    | * adc9f0c file changed completely afterwards
    | * d110262 squash S1
    |/  
    * fafd9d0 init
    ");

    // TODO: if the user now puts another dependent branch, it's breaking down in many ways.
    //       We should be smarter about that and flesh out additional steps on top.
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
                    id: Some(
                        00000000-0000-0000-0000-000000000000,
                    ),
                    base: Some(
                        Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                    ),
                    segments: [
                        ref_info::ui::Segment {
                            id: NodeIndex(3),
                            ref_name: "refs/heads/S1",
                            remote_tracking_ref_name: "refs/remotes/origin/S1",
                            commits: [
                                LocalCommit(314cacb, "two\n", integrated(d110262)),
                                LocalCommit(3234835, "one\n", integrated(d110262)),
                            ],
                            commits_on_remote: [
                                Commit(16d0628, "add other remote file\n"),
                                Commit(66fe1d7, "add remote file\n"),
                            ],
                            commits_outside: None,
                            metadata: Branch,
                            push_status: Integrated,
                            base: "fafd9d0",
                        },
                    ],
                },
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
                            ref_name: "refs/heads/local",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(1af5d57, "new local file\n", integrated(ca783a4)),
                            ],
                            commits_on_remote: [],
                            commits_outside: None,
                            metadata: "None",
                            push_status: CompletelyUnpushed,
                            base: "de02b20",
                        },
                        ref_info::ui::Segment {
                            id: NodeIndex(6),
                            ref_name: "refs/heads/local-bottom",
                            remote_tracking_ref_name: "None",
                            commits: [
                                LocalCommit(de02b20, "new local-bottom file\n", integrated(ca783a4)),
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
                    segment_index: NodeIndex(1),
                    commits_ahead: 7,
                },
            ),
            extra_target: None,
            lower_bound: Some(
                NodeIndex(7),
            ),
            is_managed_ref: true,
            is_managed_commit: true,
            is_entrypoint: true,
        },
    )
    "#);
    Ok(())
}

pub fn step(
    name: &str,
) -> anyhow::Result<(
    gix::Repository,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    String,
)> {
    named_read_only_in_memory_scenario_with_description("journey01", name)
}
