use std::collections::HashMap;

use but_forge::ForgeReview;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::upstream_integration::{BranchStatus, StackStatuses, TreeStatus};
use gitbutler_reference::Refname;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn rebase_commit() {
    let Test { repo, ctx, .. } = &Test::default();

    // make sure we have an undiscovered commit in the remote branch
    {
        fs::write(repo.path().join("file.txt"), "one").unwrap();
        fs::write(repo.path().join("another_file.txt"), "").unwrap();
        let first_commit_oid = repo.commit_all("first");
        fs::write(repo.path().join("file.txt"), "two").unwrap();
        repo.commit_all("second");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let mut stack_1_id = {
        // create a branch with some commited work
        let stack_entry_1 = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest::default(),
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();
        fs::write(repo.path().join("another_file.txt"), "virtual").unwrap();

        gitbutler_branch_actions::create_commit(ctx, stack_entry_1.id, "virtual commit", None)
            .unwrap();

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].0, stack_entry_1.id);
        assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);

        stack_entry_1.id
    };

    let unapplied_branch = {
        // unapply first vbranch
        let unapplied_branch =
            gitbutler_branch_actions::unapply_stack(ctx, stack_1_id, Vec::new()).unwrap();

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "one"
        );

        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);

        Refname::from_str(&unapplied_branch).unwrap()
    };

    {
        // fetch remote
        gitbutler_branch_actions::integrate_upstream(ctx, &[], None, &Default::default()).unwrap();

        // branch is stil unapplied
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 0);

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            ""
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "two"
        );
    }

    {
        // apply first vbranch again
        let outcome = gitbutler_branch_actions::create_virtual_branch_from_branch(
            ctx,
            &unapplied_branch,
            None,
            None,
        )
        .unwrap();

        stack_1_id = outcome.0;

        // it should be rebased
        let stacks = stack_details(ctx);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].0, stack_1_id);
        assert_eq!(stacks[0].1.branch_details[0].commits.len(), 1);
        assert!(!stacks[0].1.branch_details[0].is_conflicted);

        assert_eq!(
            fs::read_to_string(repo.path().join("another_file.txt")).unwrap(),
            "virtual"
        );

        assert_eq!(
            fs::read_to_string(repo.path().join("file.txt")).unwrap(),
            "two"
        );
    }
}

#[test]
fn upstream_integration_status_without_review_map() {
    let Test { repo, ctx, .. } = &Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // Create a virtual branch with a commit
    let stack_id = {
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "feature commit", None)
            .unwrap();

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
            assert_eq!(statuses[0].1.tree_status, TreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SaflyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_merged_pr() {
    let Test { repo, ctx, .. } = &Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // Create a virtual branch with a commit
    let (stack_id, commit_id) = {
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        let commit_id =
            gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "feature commit", None)
                .unwrap();

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
            assert_eq!(statuses[0].1.tree_status, TreeStatus::Empty);
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
    let Test { repo, ctx, .. } = &Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // Create a virtual branch with a commit
    let stack_id = {
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "feature commit", None)
            .unwrap();

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
            assert_eq!(statuses[0].1.tree_status, TreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SaflyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_closed_but_not_merged_pr() {
    let Test { repo, ctx, .. } = &Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // Create a virtual branch with a commit
    let stack_id = {
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "feature commit", None)
            .unwrap();

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
            assert_eq!(statuses[0].1.tree_status, TreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SaflyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}

#[test]
fn upstream_integration_status_with_different_branch_pr() {
    let Test { repo, ctx, .. } = &Test::default();

    // Setup: Create a remote branch with commits
    {
        fs::write(repo.path().join("file.txt"), "initial").unwrap();
        let first_commit_oid = repo.commit_all("initial commit");
        fs::write(repo.path().join("file.txt"), "second").unwrap();
        repo.commit_all("second commit");
        repo.push();
        repo.reset_hard(Some(first_commit_oid));
    }

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // Create a virtual branch with a commit
    let stack_id = {
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            ctx,
            &BranchCreateRequest {
                name: Some("feature-branch".to_string()),
                ..Default::default()
            },
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        fs::write(repo.path().join("feature-file.txt"), "feature work").unwrap();
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "feature commit", None)
            .unwrap();

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
            assert_eq!(statuses[0].1.tree_status, TreeStatus::Empty);
            assert_eq!(statuses[0].1.branch_statuses.len(), 1);
            assert_eq!(statuses[0].1.branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].1.branch_statuses[0].status,
                BranchStatus::SaflyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired status"),
    }
}
