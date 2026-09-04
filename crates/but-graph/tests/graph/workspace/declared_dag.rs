//! Stacks whose DECLARED shape is a DAG: a branch naming several parents, rather than the
//! single-parent chain a stack usually is.
//!
//! Toml metadata cannot express `parents` (its read path rebuilds branches
//! without them), so these tests declare workspaces on [`InMemoryRefMetadata`]
//! directly and drive [`Workspace::from_head`] with it.

use but_core::ref_metadata::{
    StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
};
use but_graph::Workspace;
use but_testsupport::{InMemoryRefMetadata, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::walk::utils::{named_read_only_in_memory_scenario, standard_options};

fn branch(name: &str, parents: Option<&[&str]>) -> anyhow::Result<WorkspaceStackBranch> {
    Ok(WorkspaceStackBranch {
        ref_name: name.try_into()?,
        archived: false,
        parents: parents
            .map(|parents| {
                parents
                    .iter()
                    .map(|p| gix::refs::FullName::try_from(*p))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?,
    })
}

#[test]
fn diamond() -> anyhow::Result<()> {
    let (repo, toml_meta) = named_read_only_in_memory_scenario("claim-dag", "diamond")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 78f253f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
*   8c074d5 (top) M1
|\  
| * 5996d57 (right) R1
* | 23af8d5 (left) L1
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let mut meta = InMemoryRefMetadata::default();
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let mut ws_md = but_core::RefMetadata::workspace(&meta, ws_ref.as_ref())?;
    ws_md.stacks = vec![WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: vec![
            branch(
                "refs/heads/top",
                Some(&["refs/heads/left", "refs/heads/right"]),
            )?,
            branch("refs/heads/left", Some(&[]))?,
            branch("refs/heads/right", Some(&[]))?,
        ],
    }];
    but_core::RefMetadata::set_workspace(&mut meta, &ws_md)?;

    let _ = toml_meta;
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?;
    assert_eq!(
        ws.stacks[0].edges,
        [(0, 1), (0, 2)],
        "the declared fork edges travel into the segment graph's stored shape"
    );
    // Each leg's base is its DAG parent's territory, not its list neighbor —
    // the constructor must not re-thread fork bases by adjacency.
    assert_eq!(
        ws.stacks[0]
            .segments
            .iter()
            .map(|seg| seg.base.map(|id| id.to_string()[..7].to_string()))
            .collect::<Vec<_>>(),
        [
            Some("23af8d5".into()),
            Some("965998b".into()),
            Some("965998b".into())
        ],
        "top rests on left's tip; both legs rest on the shared base"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 965998b
└── ≡:top on 965998b {1}
    ├── :top
    │   └── ·8c074d5 (🏘️)
    ├── :left
    │   └── ·23af8d5 (🏘️)
    └── :right
        └── ·5996d57 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn fork_with_empty_leg() -> anyhow::Result<()> {
    let (repo, _toml_meta) = named_read_only_in_memory_scenario("claim-dag", "fork-empty-leg")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 78f253f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
*   8c074d5 (top) M1
|\  
| * 5996d57 (right) R1
* | 23af8d5 (mid, left) L1
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let mut meta = InMemoryRefMetadata::default();
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let mut ws_md = but_core::RefMetadata::workspace(&meta, ws_ref.as_ref())?;
    ws_md.stacks = vec![WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: vec![
            branch(
                "refs/heads/top",
                Some(&["refs/heads/mid", "refs/heads/right"]),
            )?,
            branch("refs/heads/mid", Some(&["refs/heads/left"]))?,
            branch("refs/heads/left", Some(&[]))?,
            branch("refs/heads/right", Some(&[]))?,
        ],
    }];
    but_core::RefMetadata::set_workspace(&mut meta, &ws_md)?;

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?;
    assert_eq!(
        ws.stacks[0].edges,
        [(0, 1), (0, 3), (1, 2)],
        "the empty mid splices on its DECLARED edge between top and left"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 965998b
└── ≡:top on 965998b {1}
    ├── :top
    │   └── ·8c074d5 (🏘️)
    ├── :mid
    ├── :left
    │   └── ·23af8d5 (🏘️)
    └── :right
        └── ·5996d57 (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn fork_with_anonymous_leg() -> anyhow::Result<()> {
    let (repo, _toml_meta) = named_read_only_in_memory_scenario("claim-dag", "fork-anon-leg")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 78f253f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
*   8c074d5 (top) M1
|\  
| * 5996d57 R1
* | 23af8d5 (left) L1
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );

    let mut meta = InMemoryRefMetadata::default();
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let mut ws_md = but_core::RefMetadata::workspace(&meta, ws_ref.as_ref())?;
    ws_md.stacks = vec![WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: vec![
            branch("refs/heads/top", Some(&["refs/heads/left"]))?,
            branch("refs/heads/left", Some(&[]))?,
        ],
    }];
    but_core::RefMetadata::set_workspace(&mut meta, &ws_md)?;

    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        ..Default::default()
    };
    let ws = Workspace::from_head(
        &repo,
        &meta,
        project_meta,
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?;
    // v0 semantics, pinned: the anonymous interior ABSORBS into the claiming
    // child's extent (exactly how chains absorb unnamed commits into the segment
    // above). The I5 refinement — a separate anon segment hanging on the merge
    // edge — is the recorded open design in dag-stacks-plan.md step 2.
    assert_eq!(ws.stacks[0].edges, [(0, 1)]);
    assert_eq!(
        ws.stacks[0].segments[0]
            .commits
            .iter()
            .map(|id| id.to_string()[..7].to_string())
            .collect::<Vec<_>>(),
        ["8c074d5", "5996d57"],
        "the anonymous interior R1 absorbs into top's extent"
    );
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 965998b
└── ≡:top on 965998b {1}
    ├── :top
    │   ├── ·8c074d5 (🏘️)
    │   └── ·5996d57 (🏘️)
    └── :left
        └── ·23af8d5 (🏘️)

"#]]
    );
    Ok(())
}
