use but_graph::Graph;
use but_graph::target_ref_relations::{FirstParentTraversal, HeadStatus};

use crate::init::{add_workspace, read_only_in_memory_scenario, standard_options};

fn id(repo: &gix::Repository, spec: &str) -> gix::ObjectId {
    repo.rev_parse_single(spec).expect("valid revspec").detach()
}

/// Build a workspace graph from a scenario and compute upstream commits against `target_ref`.
fn upstream(
    scenario: &str,
    target_ref: &str,
    workspace: bool,
    first_parent: FirstParentTraversal,
) -> anyhow::Result<(gix::Repository, Vec<HeadStatus>)> {
    let (repo, mut meta) = read_only_in_memory_scenario(scenario)?;
    if workspace {
        add_workspace(&mut meta);
    }
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let target: gix::refs::FullName = target_ref.try_into()?;
    let results = graph.upstream_commits(&repo, target.as_ref(), first_parent)?;
    Ok((repo, results))
}

#[test]
fn workspace_with_target_ahead_returns_upstream_commits() -> anyhow::Result<()> {
    let (repo, results) = upstream(
        "ws/proper-remote-ahead",
        "refs/remotes/origin/main",
        true,
        FirstParentTraversal::No,
    )?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].head, id(&repo, "main"));
    assert_eq!(results[0].upstream_commits.len(), 2);
    assert!(
        results[0]
            .upstream_commits
            .contains(&id(&repo, "origin/main"))
    );
    assert!(
        results[0]
            .upstream_commits
            .contains(&id(&repo, "origin/main~1"))
    );
    Ok(())
}

#[test]
fn workspace_up_to_date_returns_empty_upstream() -> anyhow::Result<()> {
    let (_repo, results) = upstream(
        "ws/on-top-of-target-with-history",
        "refs/remotes/origin/main",
        true,
        FirstParentTraversal::No,
    )?;

    assert_eq!(results.len(), 1);
    assert!(results[0].upstream_commits.is_empty());
    Ok(())
}

#[test]
fn first_parent_traversal_filters_merge_parents() -> anyhow::Result<()> {
    let (_repo, results_all) = upstream(
        "ws/deduced-remote-ahead",
        "refs/remotes/origin/A",
        true,
        FirstParentTraversal::No,
    )?;
    let (_repo, results_fp) = upstream(
        "ws/deduced-remote-ahead",
        "refs/remotes/origin/A",
        true,
        FirstParentTraversal::Yes,
    )?;

    assert_eq!(results_all.len(), results_fp.len());
    let (all, fp) = (
        results_all[0].upstream_commits.len(),
        results_fp[0].upstream_commits.len(),
    );
    assert!(
        fp < all,
        "first-parent ({fp}) should yield fewer than full ({all})"
    );
    Ok(())
}

#[test]
fn non_workspace_entrypoint_uses_head_as_single_entry() -> anyhow::Result<()> {
    let (repo, results) = upstream(
        "only-remote-advanced",
        "refs/remotes/origin/main",
        false,
        FirstParentTraversal::No,
    )?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].head, id(&repo, "HEAD"));
    assert_eq!(results[0].upstream_commits.len(), 2);
    Ok(())
}

#[test]
fn multiple_stacks_different_distances() -> anyhow::Result<()> {
    let (repo, results) = upstream(
        "ws/multi-lane-with-shared-segment",
        "refs/remotes/origin/main",
        true,
        FirstParentTraversal::No,
    )?;

    assert_eq!(results.len(), 3);
    for status in &results {
        assert!(!status.upstream_commits.is_empty());
        assert!(status.upstream_commits.contains(&id(&repo, "origin/main")));
    }
    Ok(())
}

#[test]
fn integrated_branch_upstream_commits() -> anyhow::Result<()> {
    let (repo, results) = upstream(
        "ws/multi-lane-with-shared-segment-one-integrated",
        "refs/remotes/origin/main",
        true,
        FirstParentTraversal::No,
    )?;

    assert_eq!(results.len(), 3);
    let a_head = id(&repo, "A");
    let a_status = results
        .iter()
        .find(|h| h.head == a_head)
        .expect("should have A head");
    let non_a_max = results
        .iter()
        .filter(|h| h.head != a_head)
        .map(|h| h.upstream_commits.len())
        .max()
        .unwrap();

    assert!(a_status.upstream_commits.len() <= non_a_max);
    assert!(!a_status.upstream_commits.is_empty());
    Ok(())
}
