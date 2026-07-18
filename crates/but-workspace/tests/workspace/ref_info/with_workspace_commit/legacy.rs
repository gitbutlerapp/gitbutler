#![expect(deprecated, reason = "covers calls to stacks_v3 and stack_details_v3")]

mod stacks {
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::legacy::StacksFilter;
    use snapbox::prelude::*;

    use crate::ref_info::{
        stacks_v3,
        with_workspace_commit::{
            read_only_in_memory_scenario,
            utils::{StackState, add_stack},
        },
    };

    #[test]
    fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
    -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;

        add_stack(&mut meta, 1, "B-on-A", StackState::InWorkspace);
        add_stack(&mut meta, 2, "C-on-A", StackState::Inactive);
        add_stack(
            &mut meta,
            3,
            "does-not-exist-inactive",
            StackState::Inactive,
        );
        add_stack(
            &mut meta,
            4,
            "does-not-exist-active",
            StackState::InWorkspace,
        );
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   820f2b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 4e5484a (B-on-A) add new file in B-on-A
* | 5f37dbf (C-on-A) add new file in C-on-A
|/  
| * 89cc2d3 (origin/A) change in A
|/  
* d79bba9 (A) new file in A
* c166d42 (origin/main, origin/HEAD, main) init-integration

"#]]
            .raw()
        );
        // It's notable that the segment A is shared between both stacks.
        let mut actual = stacks_v3(&repo, &meta, StacksFilter::All, None)?;
        actual.sort_by(|left, right| {
            right
                .heads
                .first()
                .map(|head| &head.name)
                .cmp(&left.heads.first().map(|head| &head.name))
        });
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
[
    StackEntry {
        id: Some(
            00000000-0000-0000-0000-000000000002,
        ),
        heads: [
            StackHeadInfo {
                name: "C-on-A",
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "A",
                tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
        order: None,
        is_checked_out: false,
    },
    StackEntry {
        id: Some(
            00000000-0000-0000-0000-000000000001,
        ),
        heads: [
            StackHeadInfo {
                name: "B-on-A",
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "A",
                tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
        order: None,
        is_checked_out: false,
    },
]

"#]]
        );

        let mut actual = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
        actual.sort_by(|left, right| {
            right
                .heads
                .first()
                .map(|head| &head.name)
                .cmp(&left.heads.first().map(|head| &head.name))
        });
        // It lists both still as both are reachable from a workspace commit, so clearly in the workspace.
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
[
    StackEntry {
        id: Some(
            00000000-0000-0000-0000-000000000002,
        ),
        heads: [
            StackHeadInfo {
                name: "C-on-A",
                tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "A",
                tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
        order: None,
        is_checked_out: false,
    },
    StackEntry {
        id: Some(
            00000000-0000-0000-0000-000000000001,
        ),
        heads: [
            StackHeadInfo {
                name: "B-on-A",
                tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
                review_id: None,
                is_checked_out: false,
            },
            StackHeadInfo {
                name: "A",
                tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                review_id: None,
                is_checked_out: false,
            },
        ],
        tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
        order: None,
        is_checked_out: false,
    },
]

"#]]
        );

        let actual = stacks_v3(&repo, &meta, StacksFilter::Unapplied, None)?;
        // nothing reachable
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
[]

"#]]
        );

        Ok(())
    }
}

mod stack_details {
    use but_testsupport::{invoke_bash, visualize_commit_graph_all};
    use snapbox::prelude::*;

    use crate::ref_info::{
        head_info, stack_details_v3,
        utils::standard_options,
        with_workspace_commit::{
            read_only_in_memory_scenario,
            utils::named_writable_scenario,
            utils::{StackState, add_stack, add_stack_with_segments},
        },
    };

    #[test]
    fn simple_fully_pushed() -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario(
            "three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependent",
        )?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* f8f33a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* cbc6713 (origin/advanced-lane, on-top-of-dependent, dependent, advanced-lane) change
* fafd9d0 (origin/main, main, lane) init

"#]]
        );

        let stack_id = add_stack_with_segments(
            &mut meta,
            1,
            "dependent",
            StackState::InWorkspace,
            &["advanced-lane"],
        );
        let actual = stack_details_v3(stack_id.into(), &repo, &meta)?;
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
StackDetails {
    derived_name: "dependent",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "dependent",
            reference: FullName(
                "refs/heads/dependent",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
            base_commit: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [],
            is_conflicted: false,
            commits: [],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "advanced-lane",
            reference: FullName(
                "refs/heads/advanced-lane",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: Some(
                "refs/remotes/origin/advanced-lane",
            ),
            pr_number: None,
            review_id: None,
            tip: Sha1(cbc6713ccfc78aa9a3c9cf8305a6fadce0bbe1a4),
            base_commit: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
            push_status: NothingToPush,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(cbc6713, "change", local/remote(identity)),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
        );
        Ok(())
    }

    #[test]
    fn multiple_branches_with_shared_segment_automatically_know_containing_workspace()
    -> anyhow::Result<()> {
        let (repo, mut meta) = read_only_in_memory_scenario("multiple-stacks-with-shared-segment")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   820f2b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 4e5484a (B-on-A) add new file in B-on-A
* | 5f37dbf (C-on-A) add new file in C-on-A
|/  
| * 89cc2d3 (origin/A) change in A
|/  
* d79bba9 (A) new file in A
* c166d42 (origin/main, origin/HEAD, main) init-integration

"#]]
            .raw()
        );

        let b_stack_id = add_stack(&mut meta, 1, "B-on-A", StackState::InWorkspace);
        let c_stack_id = add_stack(&mut meta, 2, "C-on-A", StackState::InWorkspace);
        let actual = stack_details_v3(Some(b_stack_id), &repo, &meta)?;
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
StackDetails {
    derived_name: "B-on-A",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "B-on-A",
            reference: FullName(
                "refs/heads/B-on-A",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(4e5484ac0f1da1909414b1e16bd740c1a3599509),
            base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(4e5484a, "add new file in B-on-A", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "A",
            reference: FullName(
                "refs/heads/A",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: Some(
                "refs/remotes/origin/A",
            ),
            pr_number: None,
            review_id: None,
            tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
            base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
            push_status: UnpushedCommitsRequiringForce,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(d79bba9, "new file in A", local/remote(identity)),
            ],
            upstream_commits: [
                UpstreamCommit(89cc2d3, "change in A"),
            ],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
        );

        let actual = stack_details_v3(Some(c_stack_id), &repo, &meta)?;
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
StackDetails {
    derived_name: "C-on-A",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "C-on-A",
            reference: FullName(
                "refs/heads/C-on-A",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(5f37dbfd4b1c3d2ee75f216665ab4edf44c843cb),
            base_commit: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(5f37dbf, "add new file in C-on-A", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "A",
            reference: FullName(
                "refs/heads/A",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: Some(
                "refs/remotes/origin/A",
            ),
            pr_number: None,
            review_id: None,
            tip: Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
            base_commit: Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
            push_status: UnpushedCommitsRequiringForce,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(d79bba9, "new file in A", local/remote(identity)),
            ],
            upstream_commits: [
                UpstreamCommit(89cc2d3, "change in A"),
            ],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
        );
        Ok(())
    }

    #[test]
    fn multi_segment_stack_uses_advanced_tip_ref_to_find_full_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-one-stack")?;
        let stack_id = add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

        invoke_bash(
            r#"
            git checkout B
            git commit --allow-empty -m B-outside
            git checkout gitbutler/workspace
            "#,
            &repo,
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* cc0bf57 (B) B-outside
| * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* d69fe94 B
* 09d8e52 (A) A
* 85efbe4 (origin/main, main) M

"#]]
        );

        // The advanced tip must not make the lower branch disappear from the workspace view.
        let info = head_info(&repo, &meta, standard_options())?;
        assert!(
            info.stacks
                .iter()
                .flat_map(|stack| &stack.segments)
                .any(|segment| segment
                    .ref_info
                    .as_ref()
                    .is_some_and(|ref_info| { ref_info.ref_name.as_bstr() == b"refs/heads/A" }))
        );

        // Looking up by `stack_id` now prefers the current `HEAD` projection if it can still see
        // that stack, and only falls back to resolving from a surviving ref when `HEAD` cannot.
        // That keeps the stack anchored in the same workspace view as `head_info()`, so `B` stays
        // the top segment instead of being re-anchored from `refs/heads/B`.
        // Legacy `StackDetails` still has no dedicated `commits_outside` field and continues to
        // discard those commits entirely, so `B-outside` is intentionally omitted here.
        let actual = stack_details_v3(Some(stack_id), &repo, &meta)?;
        snapbox::assert_data_eq!(
            actual.to_debug(),
            snapbox::str![[r#"
StackDetails {
    derived_name: "B",
    push_status: CompletelyUnpushed,
    branch_details: [
        BranchDetails {
            name: "B",
            reference: FullName(
                "refs/heads/B",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(d69fe9427ac4a2422ab953acba483f804e8098ef),
            base_commit: Sha1(09d8e528cc9381ddc4a7a436d83507b20fc909b0),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(d69fe94, "B", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
        BranchDetails {
            name: "A",
            reference: FullName(
                "refs/heads/A",
            ),
            linked_worktree_id: None,
            remote_tracking_branch: None,
            pr_number: None,
            review_id: None,
            tip: Sha1(09d8e528cc9381ddc4a7a436d83507b20fc909b0),
            base_commit: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
            push_status: CompletelyUnpushed,
            last_updated_at: None,
            authors: [
                author <author@example.com>,
            ],
            is_conflicted: false,
            commits: [
                Commit(09d8e52, "A", local),
            ],
            upstream_commits: [],
            is_remote_head: false,
        },
    ],
    is_conflicted: false,
}

"#]]
        );
        Ok(())
    }
}
