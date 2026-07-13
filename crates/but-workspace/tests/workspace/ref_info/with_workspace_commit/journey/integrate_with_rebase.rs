//! A loose collection of states that users typically encounter.
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::visualize_commit_graph_all;
use snapbox::prelude::*;

use crate::ref_info::{
    utils::standard_options,
    with_workspace_commit::{
        head_info, utils::named_read_only_in_memory_scenario_with_description,
    },
};

#[test]
fn two_commits_rebased_onto_target() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-one-rewritten-one-local-after-push")?;
    snapbox::assert_data_eq!(
        description,
        snapbox::str![[r#"
two local commits pushed to a remote, then rebased onto target.

The branch should then be considered integrated

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    let info = head_info(&repo, &meta, standard_options());
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
Ok(
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/gitbutler/workspace",
                ),
                commit_id: Some(
                    Sha1(946cdb70e5c527a30bf8154b445c188908d25806),
                ),
                worktree: Some(
                    Worktree {
                        kind: Main,
                        owned_by_repo: true,
                    },
                ),
            },
        ),
        symbolic_remote_names: {
            "origin",
        },
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
                            LocalCommit(f9c2b14, "A2\n", integrated(36c87e5)),
                            LocalCommit(e1f216e, "A1\n", integrated(818dbb2)),
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
        target_ref: Some(
            TargetRef {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 5,
            },
        ),
        target_commit: Some(
            TargetCommit {
                commit_id: Sha1(eabf2989a998260c7fbe181b33d5772705d62907),
                segment_index: NodeIndex(2),
            },
        ),
        lower_bound: Some(
            NodeIndex(5),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    },
)

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn two_commits_rebased_onto_target_one_amended_afterwards() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-with-local-amended-after-integration")?;
    snapbox::assert_data_eq!(
        description,
        snapbox::str![[r#"
two local commits pushed to a remote, then rebased onto target, and local amended

The branch should then *not* be considered integrated anymore as A2 has changed

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 4c3a992 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 5039218 (origin/A, A) A2
* e1f216e A1
| * d89aadb (origin/main, main) M3
| * 688cfeb M2
| * 4e498f9 A2
| * d72fd2d A1
| * 7e89ffe M1
|/  
* fafd9d0 init

"#]]
    );

    let info = head_info(&repo, &meta, standard_options());
    // TODO: A2 shouldn't be integrated.
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
Ok(
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/gitbutler/workspace",
                ),
                commit_id: Some(
                    Sha1(4c3a992a5090077f9a5d35df22e065360c35729d),
                ),
                worktree: Some(
                    Worktree {
                        kind: Main,
                        owned_by_repo: true,
                    },
                ),
            },
        ),
        symbolic_remote_names: {
            "origin",
        },
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
                            LocalCommit(5039218, "A2\n", local/remote(identity)),
                            LocalCommit(e1f216e, "A1\n", integrated(d72fd2d)),
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
        target_ref: Some(
            TargetRef {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 5,
            },
        ),
        target_commit: Some(
            TargetCommit {
                commit_id: Sha1(d89aadb67d5c32e6a63cad3d36020b5e8e192a91),
                segment_index: NodeIndex(2),
            },
        ),
        lower_bound: Some(
            NodeIndex(5),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    },
)

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn two_rewritten_commits_track_as_local_and_remote() -> anyhow::Result<()> {
    let (repo, meta, description) = scenario("01-rewritten-local-commit-is-paired-with-remote")?;
    snapbox::assert_data_eq!(
        description,
        snapbox::str![[r#"
two local commits pushed to a remote, then changed locally.

One is changed locally and matched by message, the other one is matched by change-id
as the content is too different.

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0b1ed50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* e9c9d74 (A) A2
* 550b6ac A1
| * ad92cce (origin/A) A2
| * e1f216e A1
|/  
* fafd9d0 (origin/main, main) init

"#]]
    );

    let info = head_info(&repo, &meta, standard_options());
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
Ok(
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/gitbutler/workspace",
                ),
                commit_id: Some(
                    Sha1(0b1ed50b03e220f38d6d0930980512dc10bc9ab9),
                ),
                worktree: Some(
                    Worktree {
                        kind: Main,
                        owned_by_repo: true,
                    },
                ),
            },
        ),
        symbolic_remote_names: {
            "origin",
        },
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
                            LocalCommit(e9c9d74, "A2\n", local/remote(ad92cce)),
                            LocalCommit(550b6ac, "A1\n", local/remote(e1f216e)),
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
        target_ref: Some(
            TargetRef {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 0,
            },
        ),
        target_commit: Some(
            TargetCommit {
                commit_id: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
                segment_index: NodeIndex(2),
            },
        ),
        lower_bound: Some(
            NodeIndex(2),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    },
)

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn two_commits_rebased_onto_target_with_changeset_check() -> anyhow::Result<()> {
    let (repo, meta, description) =
        scenario("01-one-rewritten-one-local-after-push-author-date-change")?;
    snapbox::assert_data_eq!(
        description,
        snapbox::str![[r#"
two local commits pushed to a remote, then rebased onto target, but with the author date adjusted.

This prevents quick-checks to work.

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
    );

    let info = head_info(&repo, &meta, standard_options());
    snapbox::assert_data_eq!(
        info.to_debug(),
        snapbox::str![[r#"
Ok(
    RefInfo {
        workspace_ref_info: Some(
            RefInfo {
                ref_name: FullName(
                    "refs/heads/gitbutler/workspace",
                ),
                commit_id: Some(
                    Sha1(f1caa513c52daf127b94c157060d1ad7f911b1b1),
                ),
                worktree: Some(
                    Worktree {
                        kind: Main,
                        owned_by_repo: true,
                    },
                ),
            },
        ),
        symbolic_remote_names: {
            "origin",
        },
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
                            LocalCommit(f9c2b14, "A2\n", integrated(85063c1)),
                            LocalCommit(e1f216e, "A1\n", integrated(444639d)),
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
        target_ref: Some(
            TargetRef {
                ref_name: FullName(
                    "refs/remotes/origin/main",
                ),
                segment_index: NodeIndex(1),
                commits_ahead: 5,
            },
        ),
        target_commit: Some(
            TargetCommit {
                commit_id: Sha1(7a2d071f19ec7551996099943167460ff2c2dd9d),
                segment_index: NodeIndex(2),
            },
        ),
        lower_bound: Some(
            NodeIndex(5),
        ),
        is_managed_ref: true,
        is_managed_commit: true,
        ancestor_workspace_commit: None,
        is_entrypoint: true,
    },
)

"#]]
        .raw()
    );
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
