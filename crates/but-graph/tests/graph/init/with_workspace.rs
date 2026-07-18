use but_core::{RefMetadata, WORKSPACE_REF_NAME, ref_metadata::ProjectMeta};
use but_graph::{Graph, NodeKind, init::Overlay, workspace::WorkspaceKind};

use super::read_only_in_memory_scenario;
use super::utils::{StackState, add_stack_with_segments, add_workspace};

fn project_meta(meta: &impl RefMetadata) -> ProjectMeta {
    meta.workspace(WORKSPACE_REF_NAME.try_into().expect("valid workspace ref"))
        .map(|workspace| workspace.project_meta())
        .unwrap_or_default()
}

#[test]
fn managed_workspace_projects_its_stacks() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_workspace(&mut meta);
    let graph = Graph::from_repo(&repo, &*meta, project_meta(&*meta), Overlay::default())?;
    let managed_id = repo.rev_parse_single("gitbutler/workspace")?.detach();

    assert_eq!(graph.managed_workspace_commit_id(), Some(managed_id));
    let workspace = graph.into_workspace()?;
    assert!(matches!(workspace.kind, WorkspaceKind::Managed { .. }));
    assert!(!workspace.stacks.is_empty());
    Ok(())
}

#[test]
fn workspace_entrypoint_detects_its_managed_commit_without_metadata() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    let managed_id = repo.rev_parse_single("gitbutler/workspace")?.detach();
    let graph = Graph::from_repo(&repo, &*meta, Default::default(), Overlay::default())?;

    assert_eq!(graph.managed_workspace_commit_id(), Some(managed_id));
    Ok(())
}

#[test]
fn advanced_metadata_stack_tip_enriches_the_physical_workspace_parent() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/advanced-stack-tip-outside-workspace")?;
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);
    let workspace = Graph::from_repo(&repo, &*meta, project_meta(&*meta), Overlay::default())?
        .into_workspace()?;

    assert_eq!(workspace.stacks.len(), 1);
    assert_eq!(
        workspace.stacks[0].ref_name().map(ToString::to_string),
        Some("refs/heads/B".to_owned())
    );
    for revision in ["B~1", "A"] {
        assert!(
            workspace.contains_commit(repo.rev_parse_single(revision)?.detach()),
            "the managed workspace parent range includes {revision}"
        );
    }
    assert!(
        !workspace.contains_commit(repo.rev_parse_single("B")?.detach()),
        "metadata does not extend workspace membership beyond the managed commit's parent"
    );
    let segment = workspace.stacks[0]
        .segments
        .first()
        .expect("the metadata ref labels the physical workspace path");
    assert_eq!(
        segment
            .commits_outside
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        [repo.rev_parse_single("B")?.detach()],
        "the advanced metadata tip remains visible as outside workspace history"
    );
    Ok(())
}

#[test]
fn disconnected_target_and_workspace_are_fully_traversed() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario(
        "ws/two-branches-one-advanced-two-parent-ws-commit-diverged-ttb",
    )?;
    add_stack_with_segments(&mut meta, 1, "lane", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "advanced-lane", StackState::InWorkspace, &[]);
    let graph = Graph::from_repo(&repo, &*meta, project_meta(&*meta), Overlay::default())?;
    let selected_commit_ids = graph
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            NodeKind::Commit { id } => Some(*id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected_commit_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        selected_commit_ids.len(),
        "each selected commit is materialized once"
    );
    assert!(
        graph
            .nodes()
            .iter()
            .all(|node| !matches!(node.kind(), NodeKind::Boundary { .. })),
        "selected workspace tips are traversed to real roots"
    );
    for root_id in [
        repo.rev_parse_single("main")?.detach(),
        repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
    ] {
        let (_, root) = graph
            .node_by_commit_id(root_id)
            .expect("each disconnected root is materialized");
        assert!(root.parents().is_empty(), "the root is not synthetic");
    }
    let workspace = graph.into_workspace()?;

    assert!(matches!(workspace.kind, WorkspaceKind::Managed { .. }));
    let stack_names = workspace
        .stacks
        .iter()
        .filter_map(|stack| stack.ref_name().map(ToString::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        stack_names,
        ["refs/heads/advanced-lane", "refs/heads/lane"]
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
    );
    assert!(workspace.contains_commit(repo.rev_parse_single("advanced-lane")?.detach()));
    Ok(())
}
