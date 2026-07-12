//! Lanes that fork with no declaration saying so: separately declared stacks whose tips
//! converge on a shared tail above the integration base. Convergence makes them one
//! multi-tip class, and the segments it produces are SIBLINGS — list neighbours that must
//! never be mistaken for what each other rests on.

use but_core::ref_metadata::{
    StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
};
use but_graph::Workspace;
use but_testsupport::{InMemoryRefMetadata, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::walk::utils::{named_read_only_in_memory_scenario, standard_options};

/// A stack of `ref_names` (tip→base), declared without any DAG parents — the ordinary shape.
fn stack(id: u128, ref_names: &[&str]) -> anyhow::Result<WorkspaceStack> {
    Ok(WorkspaceStack {
        id: StackId::from_number_for_testing(id),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: ref_names
            .iter()
            .map(|ref_name| {
                Ok(WorkspaceStackBranch {
                    ref_name: (*ref_name).try_into()?,
                    archived: false,
                    parents: None,
                })
            })
            .collect::<anyhow::Result<_>>()?,
    })
}

#[test]
fn converged_lanes_rest_on_the_convergence_point() -> anyhow::Result<()> {
    let (repo, _toml_meta) = named_read_only_in_memory_scenario("claim-dag", "converging-lanes")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   7fb08b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 43cd93d (B) B1
* | c44b578 (A) A1
|/  
* b58c708 S1
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let mut meta = InMemoryRefMetadata::default();
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let mut ws_md = but_core::RefMetadata::workspace(&meta, ws_ref.as_ref())?;
    ws_md.stacks = vec![stack(1, &["refs/heads/A"])?, stack(2, &["refs/heads/B"])?];
    but_core::RefMetadata::set_workspace(&mut meta, &ws_md)?;

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Workspace::from_head(&repo, &meta, project_meta, standard_options())?;

    assert_eq!(
        ws.stacks.len(),
        1,
        "tips converging above the integration base are one multi-tip stack"
    );
    let stack = &ws.stacks[0];
    assert_eq!(
        stack
            .segments
            .iter()
            .map(|s| s.ref_name().map(|n| n.shorten().to_string()))
            .collect::<Vec<_>>(),
        [Some("A".into()), Some("B".into()), None],
        "the two lanes, then the shared tail they converge on"
    );
    assert_eq!(
        stack.edges,
        [(0, 2), (1, 2)],
        "both lanes are children of the shared tail, not of each other"
    );

    let shared_tip = stack.segments[2].tip();
    // A lane's list neighbour is its SIBLING. Re-threading bases by adjacency handed lane A
    // the tip of lane B, which made A's branch diff report B's commits as deletions.
    assert_eq!(
        (stack.segments[0].base, stack.segments[1].base),
        (shared_tip, shared_tip),
        "each lane rests on the convergence point"
    );
    assert_ne!(
        stack.segments[0].base,
        stack.segments[1].tip(),
        "lane A does not rest on lane B, however adjacent they are in the list"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 965998b
├── ≡:A on 965998b {1}
│   ├── :A
│   │   └── ·c44b578 (🏘️)
│   └── :anon:
│       └── ·b58c708 (🏘️)
└── ≡:B on 965998b {2}
    ├── :B
    │   └── ·43cd93d (🏘️)
    └── :anon:
        └── ·b58c708 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn an_empty_segment_resolves_through_its_own_lane() -> anyhow::Result<()> {
    let (repo, _toml_meta) =
        named_read_only_in_memory_scenario("claim-dag", "converging-lanes-empty")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   7fb08b3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 43cd93d (B) B1
* | c44b578 (E, A) A1
|/  
* b58c708 S1
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let mut meta = InMemoryRefMetadata::default();
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let mut ws_md = but_core::RefMetadata::workspace(&meta, ws_ref.as_ref())?;
    ws_md.stacks = vec![
        stack(1, &["refs/heads/E", "refs/heads/A"])?,
        stack(2, &["refs/heads/B"])?,
    ];
    but_core::RefMetadata::set_workspace(&mut meta, &ws_md)?;

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Workspace::from_head(&repo, &meta, project_meta, standard_options())?;

    let stack = &ws.stacks[0];
    assert_eq!(
        stack
            .segments
            .iter()
            .map(|s| (
                s.ref_name().map(|n| n.shorten().to_string()),
                s.commits.len()
            ))
            .collect::<Vec<_>>(),
        [
            (Some("E".into()), 0),
            (Some("A".into()), 1),
            (Some("B".into()), 1),
            (None, 1)
        ],
        "the empty sits inside its own lane, above the segment naming their shared commit"
    );

    // An empty has no tip, so a resting-commit lookup has to read PAST it — and the very next
    // list entry after a lane is the SIBLING lane. What keeps that safe is that a lane always
    // ends in a segment with commits, so an empty is never the last entry of one.
    let a_tip = stack.segments[1].tip();
    assert_eq!(
        ws.try_branch_resting_commit_id("refs/heads/E".try_into()?)?,
        a_tip.expect("A has a commit"),
        "E rests on its own lane's commit, not on lane B's tip"
    );
    assert_ne!(
        Some(ws.try_branch_resting_commit_id("refs/heads/E".try_into()?)?),
        stack.segments[2].tip(),
        "reading past the empty must not reach the sibling lane"
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 965998b
├── ≡:E on 965998b {1}
│   ├── :E
│   ├── :A
│   │   └── ·c44b578 (🏘️)
│   └── :anon:
│       └── ·b58c708 (🏘️)
└── ≡:B on 965998b {2}
    ├── :B
    │   └── ·43cd93d (🏘️)
    └── :anon:
        └── ·b58c708 (🏘️)

"#]]
    );
    Ok(())
}
