//! SPIKE (commit-graph-experiment): prove the StepGraph's rebase topology can be produced straight
//! from a commit-first `CommitGraph`, with parity against the segment-based path on real fixtures.

use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::testing::commit_graph_step_parity;

use crate::utils::{fixture, standard_options};

fn project_meta(meta: &impl but_core::RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

/// For `fixture_name`, build the rebase topology from the segment-based editor and from a
/// `CommitGraph` built off the same graph, and assert that every commit the editor picks gets the
/// same ordered parents from the commit-graph-only build.
fn assert_parity(fixture_name: &str) -> Result<()> {
    let (repo, mut meta) = fixture(fixture_name)?;
    let graph =
        Graph::from_head(&repo, &*meta, project_meta(&*meta), standard_options())?.validated()?;
    let mut ws = graph.into_workspace()?;

    let (segment_based, commit_based) = commit_graph_step_parity(&mut ws, &mut *meta, &repo)?;

    assert!(
        !segment_based.is_empty(),
        "{fixture_name}: segment-based build produced no picks"
    );
    for (commit, parents) in &segment_based {
        assert_eq!(
            commit_based.get(commit),
            Some(parents),
            "{fixture_name}: ordered parents differ for commit {commit}\n  segment-based: {parents:?}\n  commit-based:  {:?}",
            commit_based.get(commit)
        );
    }
    Ok(())
}

#[test]
fn four_commits_parity() -> Result<()> {
    assert_parity("four-commits")
}

#[test]
fn merge_in_the_middle_parity() -> Result<()> {
    assert_parity("merge-in-the-middle")
}
