use std::collections::HashMap;

use but_forge::ForgeReview;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus, Resolution, ResolutionApproach, StackStatuses, UpstreamTreeStatus,
};

use super::*;

#[test]
fn upstream_integration_status_without_review_map() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a virtual branch with a commit
    let stack_id = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        super::create_commit(ctx, stack_entry.id, "feature commit").unwrap();

        stack_entry.id
    };

    let empty_review_map = HashMap::new();
    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &empty_review_map)
            .unwrap();

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].0, Some(stack_id));
            assert_eq!(statuses[0].1.tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_merged_pr() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a virtual branch with a commit
    let (stack_id, commit_id) = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        let commit_id = super::create_commit(ctx, stack_entry.id, "feature commit").unwrap();

        (stack_entry.id, commit_id)
    };

    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        ForgeReview {
            html_url: "https://github.com/test/repo/pull/1".to_string(),
            number: 1,
            title: "Feature PR".to_string(),
            body: Some("Description".to_string()),
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature-branch".to_string(),
            target_branch: "master".to_string(),
            sha: commit_id.to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            modified_at: Some("2024-01-02T00:00:00Z".to_string()),
            merged_at: Some("2024-01-03T00:00:00Z".to_string()),
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::NaiveDateTime::parse_from_str(
                "2024-01-04 23:56:04",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
        },
    );

    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &review_map).unwrap();

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].0, Some(stack_id));
            assert_eq!(statuses[0].1.tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::Integrated
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_merged_pr_mismatched_head() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a virtual branch with a commit
    let stack_id = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        super::create_commit(ctx, stack_entry.id, "feature commit").unwrap();

        stack_entry.id
    };

    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        ForgeReview {
            html_url: "https://github.com/test/repo/pull/1".to_string(),
            number: 1,
            title: "Feature PR".to_string(),
            body: Some("Description".to_string()),
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature-branch".to_string(),
            target_branch: "master".to_string(),
            sha: "some-other-sha".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            modified_at: Some("2024-01-02T00:00:00Z".to_string()),
            merged_at: Some("2024-01-03T00:00:00Z".to_string()),
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::NaiveDateTime::parse_from_str(
                "2024-01-04 23:56:04",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
        },
    );

    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &review_map).unwrap();

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].0, Some(stack_id));
            assert_eq!(statuses[0].1.tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_closed_but_not_merged_pr() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a virtual branch with a commit
    let stack_id = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        super::create_commit(ctx, stack_entry.id, "feature commit").unwrap();

        stack_entry.id
    };

    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        ForgeReview {
            html_url: "https://github.com/test/repo/pull/1".to_string(),
            number: 1,
            title: "Feature PR".to_string(),
            body: Some("Description".to_string()),
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature-branch".to_string(),
            target_branch: "master".to_string(),
            sha: "abc123".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            modified_at: Some("2024-01-02T00:00:00Z".to_string()),
            merged_at: None,
            closed_at: Some("2024-01-03T00:00:00Z".to_string()),
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::NaiveDateTime::parse_from_str(
                "2024-01-04 23:56:04",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
        },
    );

    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &review_map).unwrap();

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].0, Some(stack_id));
            assert_eq!(statuses[0].1.tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_different_branch_pr() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a virtual branch with a commit
    let stack_id = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        super::create_commit(ctx, stack_entry.id, "feature commit").unwrap();

        stack_entry.id
    };

    let mut review_map = HashMap::new();
    review_map.insert(
        "different-branch".to_string(),
        ForgeReview {
            html_url: "https://github.com/test/repo/pull/2".to_string(),
            number: 2,
            title: "Different PR".to_string(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "different-branch".to_string(),
            target_branch: "master".to_string(),
            sha: "def456".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            modified_at: Some("2024-01-02T00:00:00Z".to_string()),
            merged_at: Some("2024-01-03T00:00:00Z".to_string()),
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::NaiveDateTime::parse_from_str(
                "2024-01-04 23:56:04",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
        },
    );

    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &review_map).unwrap();

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].0, Some(stack_id));
            assert_eq!(statuses[0].1.tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

/// Regression test: when a stack has a branch whose commits are fully integrated
/// upstream (part of the new base), `integrate_upstream` should succeed and archive
/// that branch — not fail with "The new head names do not match the current heads".
#[test]
fn integrate_upstream_with_fully_integrated_branch_in_stack() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Setup remote: create an initial commit, then add a second commit that
    // includes the same change branch1 will have, making branch1 "integrated".
    // Reset local back to the initial commit so there's an upstream delta.
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first = repo.commit_all("initial commit");

        fs::write(repo.path().join("branch1-file.txt"), "branch1 work").unwrap();
        repo.commit_all("upstream: merge branch1");
        repo.push();
        repo.reset_hard(Some(first));
    }

    // Set the base branch
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create a stack with two branches: branch1 (bottom) and branch3 (top)
    let stack_id = {
        let mut guard = ctx.exclusive_worktree_access();
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("branch1".to_string()),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .unwrap();
        drop(guard);

        // branch1 commit — same content as what's upstream
        fs::write(repo.path().join("branch1-file.txt"), "branch1 work").unwrap();
        super::create_commit(ctx, stack_entry.id, "branch1: first commit").unwrap();

        // Add branch3 on top of the stack with different work
        gitbutler_branch_actions::stack::create_branch(
            ctx,
            stack_entry.id,
            gitbutler_branch_actions::stack::CreateSeriesRequest {
                name: "branch3".to_string(),
                target_patch: None,
                preceding_head: None,
            },
        )
        .unwrap();

        fs::write(repo.path().join("branch3-file.txt"), "branch3 work").unwrap();
        super::create_commit(ctx, stack_entry.id, "branch3: first commit").unwrap();

        stack_entry.id
    };

    // Verify the stack has two branches
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.branch_details.len(), 2);

    // Mark branch1 as integrated via a merged review
    let branch1_commit = stacks[0].1.branch_details[1].commits[0].id.to_string();
    let mut review_map = HashMap::new();
    review_map.insert(
        "branch1".to_string(),
        ForgeReview {
            html_url: "https://github.com/test/repo/pull/1".to_string(),
            number: 1,
            title: "Branch1 PR".to_string(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "branch1".to_string(),
            target_branch: "master".to_string(),
            sha: branch1_commit,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            modified_at: Some("2024-01-02T00:00:00Z".to_string()),
            merged_at: Some("2024-01-03T00:00:00Z".to_string()),
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::NaiveDateTime::parse_from_str(
                "2024-01-04 23:56:04",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
        },
    );

    // Verify branch1 shows as integrated in statuses
    let statuses =
        gitbutler_branch_actions::upstream_integration_statuses(ctx, None, &review_map).unwrap();
    match &statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts: _,
        } => {
            let branch1_status = statuses[0]
                .1
                .branch_statuses
                .iter()
                .find(|s| s.name == "branch1")
                .expect("branch1 should be in statuses");
            assert_eq!(
                branch1_status.status,
                BranchStatus::Integrated,
                "branch1 should be marked as integrated"
            );
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }

    // Integrate upstream with a Rebase resolution for this stack.
    // Before the fix, this would fail with:
    //   "The new head names do not match the current heads"
    // because branch1 (fully integrated) is pruned from the rebase output
    // but was not yet archived when set_heads_from_rebase_output validated.
    let resolutions = vec![Resolution {
        stack_id,
        approach: ResolutionApproach::Rebase,
        delete_integrated_branches: false,
    }];

    gitbutler_branch_actions::integrate_upstream(ctx, &resolutions, None, &review_map)
        .expect("integrate_upstream should succeed when a branch in the stack is fully integrated");

    // After integration, branch1 should be archived and branch3 should remain
    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1, "stack should still exist");
    assert_eq!(
        stacks[0].1.branch_details.len(),
        1,
        "only branch3 should remain visible"
    );
    assert_eq!(stacks[0].1.branch_details[0].name, "branch3");
}

/// Regression test for the scenario where stacks carry different amounts of
/// upstream history, causing `merge_workspace` to fail during `integrate_upstream`.
///
/// Reproduces the real-world bug: three stacks are in the workspace, none of
/// them touch `foo.txt`, yet each stack tree carries a different version of
/// `foo.txt` because each was previously rebased onto a different intermediate
/// upstream commit. The stacks don't share a common base, so sequential
/// octopus-merging their trees against `target.tree()` produces a genuine
/// textual conflict on a file no stack ever edited.
///
/// The graph rebase commit path (used for all commit operations) never calls
/// `remerged_workspace_tree_v2`, so the inter-stack conflict goes undetected
/// during normal development. It only surfaces during `integrate_upstream`.
///
/// The fix adds a pre-flight check (`check_workspace_stacks_mergeable`) that
/// detects the conflict early and returns a descriptive error naming the
/// offending stack and file, so the user can unapply it and retry. No state
/// is mutated on failure.
#[test]
fn integrate_upstream_with_inter_stack_tree_conflict() {
    let Test { repo, ctx, .. } = &mut Test::default();

    // Initial state: foo.txt exists alongside per-stack files.
    // No stack will ever edit foo.txt directly — the conflict comes purely
    // from upstream history divergence.
    fs::write(repo.path().join("foo.txt"), "original\n").unwrap();
    fs::write(repo.path().join("file_a.txt"), "a\n").unwrap();
    fs::write(repo.path().join("file_b.txt"), "b\n").unwrap();
    fs::write(repo.path().join("file_c.txt"), "c\n").unwrap();
    let initial = repo.commit_all("initial");
    repo.push();

    // Upstream commits that progressively change foo.txt:
    // C1: rename (foo.txt = "renamed")
    fs::write(repo.path().join("foo.txt"), "renamed\n").unwrap();
    let c1 = repo.commit_all("upstream: rename foo");
    // C2: rewrite (foo.txt = multi-line, incompatible with C1 from target's perspective)
    fs::write(
        repo.path().join("foo.txt"),
        "renamed\nextra line\nmore content\n",
    )
    .unwrap();
    let c2 = repo.commit_all("upstream: rewrite foo");
    // C3: another upstream commit so the target can advance past C2.
    fs::write(repo.path().join("unrelated.txt"), "new\n").unwrap();
    repo.commit_all("upstream: unrelated file");
    repo.push();

    // Reset local back to initial so there's an upstream delta.
    repo.reset_hard(Some(initial));

    // Set the base branch (target = initial).
    let mut guard = ctx.exclusive_worktree_access();
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        guard.write_permission(),
    )
    .unwrap();
    drop(guard);

    // Create three stacks, each editing only its own file (never foo.txt).
    let stack_ids: Vec<_> = ["stack-a", "stack-b", "stack-c"]
        .iter()
        .zip(["file_a.txt", "file_b.txt", "file_c.txt"])
        .map(|(name, file)| {
            let mut guard = ctx.exclusive_worktree_access();
            let entry = gitbutler_branch_actions::create_virtual_branch(
                ctx,
                &BranchCreateRequest {
                    name: Some(name.to_string()),
                    ..Default::default()
                },
                guard.write_permission(),
            )
            .unwrap();
            drop(guard);
            fs::write(repo.path().join(file), format!("modified by {name}\n")).unwrap();
            super::create_commit(ctx, entry.id, &format!("{name}: modify {file}")).unwrap();
            entry.id
        })
        .collect();

    // Simulate a previous partial integrate: rebase each stack onto a different
    // intermediate upstream commit, without advancing the target. This gives
    // each stack tree a different version of foo.txt purely through upstream
    // history, even though no stack ever edited foo.txt.
    //
    // stack-a → rebased onto initial (foo.txt = "original")
    // stack-b → rebased onto C1      (foo.txt = "renamed")
    // stack-c → rebased onto C2      (foo.txt = "renamed\nextra line\nmore content")
    {
        let repo = ctx.repo.get().unwrap();

        let mut vb_state = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());

        let rebase_targets = [initial, c1, c2];
        let mut new_heads = Vec::new();

        for (stack_id, rebase_onto) in stack_ids.iter().zip(rebase_targets) {
            let mut stack = vb_state.get_stack(*stack_id).unwrap();
            let old_head = stack.heads.last().unwrap().head_oid(&repo).unwrap();

            let merge_base = repo.merge_base(rebase_onto, old_head).unwrap();
            let merge_base_tree = repo
                .find_commit(merge_base)
                .unwrap()
                .tree_id()
                .unwrap()
                .detach();
            let onto_tree = repo
                .find_commit(rebase_onto)
                .unwrap()
                .tree_id()
                .unwrap()
                .detach();
            let old_tree = repo
                .find_commit(old_head)
                .unwrap()
                .tree_id()
                .unwrap()
                .detach();
            let mut merge = repo
                .merge_trees(
                    merge_base_tree,
                    onto_tree,
                    old_tree,
                    repo.default_merge_labels(),
                    repo.tree_merge_options().unwrap(),
                )
                .unwrap();
            assert!(
                !merge.has_unresolved_conflicts(
                    gix::merge::tree::TreatAsUnresolved::forced_resolution()
                ),
                "cherry-pick onto intermediate upstream should be clean"
            );
            let tree_oid = merge.tree.write().unwrap().detach();
            let new_head = commit_without_signature_gix(
                &repo,
                None,
                signature_gix(SignaturePurpose::Author),
                signature_gix(SignaturePurpose::Committer),
                format!("rebased onto {}", &rebase_onto.to_string()[..8])
                    .as_str()
                    .into(),
                tree_oid,
                &[rebase_onto],
                None,
            )
            .unwrap();

            stack
                .set_stack_head(&mut vb_state, &repo, new_head)
                .unwrap();
            new_heads.push(new_head);
        }

        // Rebuild the workspace merge commit so stacks_v3 can discover all
        // three stacks from the graph. The tree used here is arbitrary —
        // merge_workspace recomputes it from stack trees.
        let target_commit = repo.find_commit(initial).unwrap();
        let parent_commits: Vec<_> = std::iter::once(&initial)
            .chain(new_heads.iter())
            .copied()
            .collect();
        let ws_tree = target_commit.tree_id().unwrap().detach();
        let ws_oid = commit_without_signature_gix(
            &repo,
            None,
            signature_gix(SignaturePurpose::Author),
            signature_gix(SignaturePurpose::Committer),
            "GitButler Workspace Commit".into(),
            ws_tree,
            &parent_commits,
            None,
        )
        .unwrap();
        repo.reference(
            "refs/heads/gitbutler/workspace",
            ws_oid,
            PreviousValue::Any,
            "test: rebuild workspace",
        )
        .unwrap();
    }

    // Integrate upstream. Before the fix, merge_workspace would bail with a
    // generic "merge conflict when computing workspace tree". Now we get a
    // descriptive error telling the user which stack and file are problematic.
    let resolutions: Vec<_> = stack_ids
        .iter()
        .map(|id| Resolution {
            stack_id: *id,
            approach: ResolutionApproach::Rebase,
            delete_integrated_branches: false,
        })
        .collect();
    let err =
        gitbutler_branch_actions::integrate_upstream(ctx, &resolutions, None, &Default::default())
            .expect_err(
                "should fail when stacks have conflicting trees from divergent upstream bases",
            );

    let msg = err.to_string();
    assert!(
        msg.contains("conflicts with other applied stacks"),
        "expected descriptive conflict error, got: {msg}"
    );
    assert!(
        msg.contains("foo.txt"),
        "error should name the conflicting file, got: {msg}"
    );
    assert!(
        msg.contains("unapply"),
        "error should suggest unapplying, got: {msg}"
    );

    // The failed integrate_upstream must leave the repository in a clean state:
    // all three stacks still applied, worktree unchanged, workspace functional.
    let vb_state = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let applied_stacks = vb_state.list_stacks_in_workspace().unwrap();
    assert_eq!(
        applied_stacks.len(),
        3,
        "all three stacks should still be applied after a failed integrate"
    );
    let mut applied_names: Vec<String> = applied_stacks.iter().map(|s| s.name()).collect();
    applied_names.sort();
    assert_eq!(
        applied_names,
        vec!["stack-a", "stack-b", "stack-c"],
        "all stacks should retain their names"
    );

    // Worktree files should be exactly as they were before the call.
    assert_eq!(
        fs::read_to_string(repo.path().join("foo.txt")).unwrap(),
        "original\n",
        "foo.txt should be untouched"
    );
    for (file, stack_name) in [
        ("file_a.txt", "stack-a"),
        ("file_b.txt", "stack-b"),
        ("file_c.txt", "stack-c"),
    ] {
        assert_eq!(
            fs::read_to_string(repo.path().join(file)).unwrap(),
            format!("modified by {stack_name}\n"),
            "{file} should be untouched"
        );
    }
}
