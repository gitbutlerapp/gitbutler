//! Behavioral invariants of the [`CommitGraph`](but_graph::CommitGraph) the traversal builds, which
//! merge-base and reachability are built on.

use crate::init::utils::{read_only_in_memory_scenario, standard_options};

/// Faithfulness must hold on *partial* graphs: under a hard limit, `remote-includes-another-remote`
/// cuts the local stack off at `e255adc`, whose parent `main` (`fafd9d0`) is then reachable only via
/// the *remote* side. The commit graph must not reconnect the cutoff tip to that
/// coincidentally-present parent and fabricate ancestry the traversal never established — so the
/// local `B` and the remote `origin/B` share no in-graph merge-base.
#[test]
fn commit_graph_does_not_fabricate_ancestry_across_a_hard_limit() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("remote-includes-another-remote")?;
    let graph = but_graph::Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options().with_hard_limit(5),
    )?;
    let cg = graph
        .commit_graph_ref()
        .expect("a traversal built a commit graph");

    let b = repo.rev_parse_single("B")?.detach();
    let origin_b = repo.rev_parse_single("origin/B")?.detach();
    // Both tips are present in the partial graph, but the local side dead-ends at the cutoff.
    assert!(cg.node(b).is_some() && cg.node(origin_b).is_some());
    assert_eq!(
        cg.merge_base(b, origin_b),
        None,
        "a cutoff tip must not be reconnected to a parent present only via another path",
    );
    Ok(())
}
