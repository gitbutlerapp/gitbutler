//! Tests for upstream integration status computation and `integrate_upstream`.
//!
//! These use the `but-graph` / `but-workspace` fixture pattern — no legacy
//! APIs (`set_base_branch`, `create_virtual_branch`, etc.) are used.

use std::collections::HashMap;

use bstr::ByteSlice as _;
use but_api::workspace::upstream_integration::{
    BranchStatus, StackSelector, StackStatuses, UpstreamTreeStatus,
};
use but_forge::ForgeReview;
use but_graph::init::Options;
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack, StackBranch, Target},
};
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_testsupport::gix_testtools::tempfile::TempDir;
use but_workspace::{BottomUpdate, BottomUpdateKind};

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

/// Load a writable fixture by name, returning the repo and metadata handle.
///
/// The metadata TOML is stored inside the `.git` directory so that it
/// survives across repo re-opens.
fn writable_scenario(
    name: &str,
) -> anyhow::Result<(TempDir, gix::Repository, VirtualBranchesTomlMetadata)> {
    let (repo, tmp) = but_testsupport::writable_scenario(name);
    let meta = VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))?;
    Ok((tmp, repo, meta))
}

/// Set the default target in metadata to `origin/main` at the given commit.
fn set_target(meta: &mut VirtualBranchesTomlMetadata, sha: gix::ObjectId) {
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "not needed in tests".to_string(),
        sha,
        push_remote_name: None,
    });
}

/// Register a stack in metadata with segment names (bottom-to-top).
///
/// `stack_name` is the top-most segment. `segments` lists any lower segments
/// in bottom-to-top order (i.e. the order they were created).
fn add_stack(
    meta: &mut VirtualBranchesTomlMetadata,
    stack_id: u128,
    stack_name: &str,
    segments: &[&str],
) -> but_core::ref_metadata::StackId {
    let branches = segments
        .iter()
        .rev()
        .map(|name| StackBranch::new_with_zero_head((*name).into(), None, None, false))
        .chain(std::iter::once(StackBranch::new_with_zero_head(
            stack_name.into(),
            None,
            None,
            false,
        )))
        .collect();
    let mut stack = Stack::new_with_just_heads(
        branches,
        meta.data().branches.len(),
        /* in_workspace */ true,
    );
    stack.order = stack_id as usize;
    let id = but_core::ref_metadata::StackId::from_number_for_testing(stack_id);
    stack.id = id;
    meta.data_mut().branches.insert(id, stack);
    id
}

/// Build a `Graph` → `Workspace` projection from the current repo state.
fn workspace(
    repo: &gix::Repository,
    meta: &VirtualBranchesTomlMetadata,
    extra_target: Option<gix::ObjectId>,
) -> anyhow::Result<but_graph::projection::Workspace> {
    let graph = but_graph::Graph::from_head(
        repo,
        meta,
        Options {
            extra_target_commit_id: extra_target,
            ..Options::limited()
        },
    )?;
    graph.into_workspace()
}

/// Helper to build a `ForgeReview` for testing.
fn make_review(
    branch: &str,
    sha: &str,
    merged_at: Option<&str>,
    closed_at: Option<&str>,
) -> ForgeReview {
    ForgeReview {
        html_url: "https://github.com/test/repo/pull/1".to_string(),
        number: 1,
        title: format!("{branch} PR"),
        body: Some("Description".to_string()),
        author: None,
        labels: vec![],
        draft: false,
        source_branch: branch.to_string(),
        target_branch: "main".to_string(),
        sha: sha.to_string(),
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        modified_at: Some("2024-01-02T00:00:00Z".to_string()),
        merged_at: merged_at.map(str::to_string),
        closed_at: closed_at.map(str::to_string),
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
    }
}

// ---------------------------------------------------------------------------
// Status tests — one stack behind upstream
// ---------------------------------------------------------------------------

/// Upstream has new commits, no review map → branch is SafelyUpdatable.
#[test]
fn status_without_review_map() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("one-stack-behind-upstream")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    let stack_id = add_stack(&mut meta, 1, "feature-branch", &[]);
    let ws = workspace(&repo, &meta, Some(m1))?;

    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &HashMap::new())?;

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].stack_id, Some(stack_id));
            assert_eq!(statuses[0].tree_status, UpstreamTreeStatus::Empty);
            assert_eq!(statuses[0].branch_statuses.len(), 1);
            assert_eq!(statuses[0].branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }
    Ok(())
}

/// Merged PR whose SHA matches the branch head → branch is Integrated.
#[test]
fn status_with_merged_pr() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("one-stack-behind-upstream")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    let stack_id = add_stack(&mut meta, 1, "feature-branch", &[]);
    let ws = workspace(&repo, &meta, Some(m1))?;

    let commit_id = repo.rev_parse_single("feature-branch")?.detach();
    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        make_review(
            "feature-branch",
            &commit_id.to_string(),
            Some("2024-01-03T00:00:00Z"),
            None,
        ),
    );

    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &review_map)?;

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].stack_id, Some(stack_id));
            assert_eq!(statuses[0].branch_statuses.len(), 1);
            assert_eq!(statuses[0].branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].branch_statuses[0].status,
                BranchStatus::Integrated
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }
    Ok(())
}

/// Merged PR whose SHA does NOT match the branch head → branch is SafelyUpdatable.
#[test]
fn status_with_merged_pr_mismatched_head() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("one-stack-behind-upstream")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    let stack_id = add_stack(&mut meta, 1, "feature-branch", &[]);
    let ws = workspace(&repo, &meta, Some(m1))?;

    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        make_review(
            "feature-branch",
            "some-other-sha",
            Some("2024-01-03T00:00:00Z"),
            None,
        ),
    );

    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &review_map)?;

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].stack_id, Some(stack_id));
            assert_eq!(statuses[0].branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }
    Ok(())
}

/// PR is closed but not merged (merged_at is None) → branch is SafelyUpdatable.
#[test]
fn status_with_closed_but_not_merged_pr() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("one-stack-behind-upstream")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    let stack_id = add_stack(&mut meta, 1, "feature-branch", &[]);
    let ws = workspace(&repo, &meta, Some(m1))?;

    let mut review_map = HashMap::new();
    review_map.insert(
        "feature-branch".to_string(),
        make_review(
            "feature-branch",
            "abc123",
            None,
            Some("2024-01-03T00:00:00Z"),
        ),
    );

    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &review_map)?;

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].stack_id, Some(stack_id));
            assert_eq!(statuses[0].branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }
    Ok(())
}

/// Merged PR for a *different* branch → our branch is SafelyUpdatable.
#[test]
fn status_with_different_branch_pr() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("one-stack-behind-upstream")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    let stack_id = add_stack(&mut meta, 1, "feature-branch", &[]);
    let ws = workspace(&repo, &meta, Some(m1))?;

    let mut review_map = HashMap::new();
    review_map.insert(
        "different-branch".to_string(),
        make_review(
            "different-branch",
            "def456",
            Some("2024-01-03T00:00:00Z"),
            None,
        ),
    );

    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &review_map)?;

    match statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts,
        } => {
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].stack_id, Some(stack_id));
            assert_eq!(statuses[0].branch_statuses[0].name, "feature-branch");
            assert_eq!(
                statuses[0].branch_statuses[0].status,
                BranchStatus::SafelyUpdatable
            );
            assert!(worktree_conflicts.is_empty());
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Integration test — two-branch stack with integrated bottom
// ---------------------------------------------------------------------------

/// A stack with 2 branches where the bottom branch is fully integrated
/// (its content matches upstream and it has a merged PR review). After
/// `integrate_upstream`, the integrated branch is pruned and only the
/// top branch remains.
#[test]
fn integrate_upstream_with_fully_integrated_branch_in_stack() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = writable_scenario("two-branch-stack-integrated-bottom")?;
    let m1 = repo.rev_parse_single("M1")?.detach();
    set_target(&mut meta, m1);
    add_stack(&mut meta, 1, "branch3", &["branch1"]);

    let ws = workspace(&repo, &meta, Some(m1))?;

    // Verify the stack has two segments.
    assert_eq!(ws.stacks.len(), 1);
    assert_eq!(ws.stacks[0].segments.len(), 2);

    // Get branch1's head commit for the review map.
    let branch1_head = repo.rev_parse_single("branch1")?.detach();

    // Mark branch1 as integrated via a merged review.
    let mut review_map = HashMap::new();
    review_map.insert(
        "branch1".to_string(),
        make_review(
            "branch1",
            &branch1_head.to_string(),
            Some("2024-01-03T00:00:00Z"),
            None,
        ),
    );

    // Verify branch1 shows as Integrated in statuses.
    let statuses =
        but_api::workspace::upstream_integration_statuses_inner(&repo, &ws, None, &review_map)?;

    let bottom_selector = match &statuses {
        StackStatuses::UpdatesRequired {
            statuses,
            worktree_conflicts: _,
        } => {
            let branch1_status = statuses[0]
                .branch_statuses
                .iter()
                .find(|s| s.name == "branch1")
                .expect("branch1 should be in statuses");
            assert_eq!(
                branch1_status.status,
                BranchStatus::Integrated,
                "branch1 should be marked as integrated"
            );
            statuses[0]
                .bottom_selector
                .as_ref()
                .expect("bottom_selector should be set")
        }
        StackStatuses::UpToDate => panic!("Expected UpdatesRequired"),
    };

    // Build the BottomUpdate from the status selector.
    let selector = match bottom_selector {
        StackSelector::Commit(oid) => RelativeTo::Commit(*oid),
        StackSelector::Reference(ref_name) => RelativeTo::Reference(ref_name.clone()),
    };

    // Integrate upstream — this should succeed even with a fully integrated branch.
    let mut ws = workspace(&repo, &meta, Some(m1))?;
    let outcome = but_workspace::integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector,
        }],
    )?;
    outcome.rebase.materialize()?;

    // Rebuild the workspace projection after integration.
    let ws = workspace(&repo, &meta, None)?;

    // After integration, branch1 should be pruned and only branch3 remains.
    assert_eq!(ws.stacks.len(), 1, "stack should still exist");
    let branch_names: Vec<String> = ws.stacks[0]
        .segments
        .iter()
        .filter_map(|s| s.ref_name().map(|r| r.shorten().to_str_lossy().to_string()))
        .collect();
    assert!(
        branch_names.contains(&"branch3".to_string()),
        "branch3 should remain, got: {branch_names:?}"
    );

    Ok(())
}
