use but_graph::{BoundaryKind, Graph, NodeGraphEntrypoint, NodeKind, init::Overlay};

mod overlay;
mod utils;
mod with_workspace;

pub use utils::{named_read_only_in_memory_scenario, read_only_in_memory_scenario};

#[test]
fn unborn_head_is_preserved() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;
    let graph = Graph::from_repo(&repo, &*meta, Default::default(), Overlay::default())?;

    assert!(graph.nodes().is_empty());
    assert!(matches!(
        graph.entrypoint(),
        NodeGraphEntrypoint::Unborn(reference)
            if reference.ref_info.ref_name.as_bstr() == b"refs/heads/main"
                && reference.ref_info.commit_id.is_none()
    ));
    assert!(graph.into_workspace()?.stacks.is_empty());
    Ok(())
}

#[test]
fn detached_head_stays_a_commit_entrypoint() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("detached")?;
    let head_id = repo.head_id()?.detach();
    let graph = Graph::from_repo(&repo, &*meta, Default::default(), Overlay::default())?;

    let NodeGraphEntrypoint::Node(index) = *graph.entrypoint() else {
        anyhow::bail!("detached HEAD must be a commit entrypoint")
    };
    assert!(matches!(graph.nodes()[index].kind(), NodeKind::Commit { id } if *id == head_id));
    assert!(graph.entrypoint_ref().is_none());
    assert!(matches!(
        graph.into_workspace()?.kind,
        but_graph::workspace::WorkspaceKind::AdHoc
    ));
    Ok(())
}

#[test]
fn shallow_parent_is_an_explicit_boundary() -> anyhow::Result<()> {
    let (repo, meta) =
        named_read_only_in_memory_scenario("special-conditions", "shallow-clone-depth-2")?;
    let shallow_id = repo.shallow_commits()?.expect("shallow clone").head;
    let graph = Graph::from_repo(&repo, &*meta, Default::default(), Overlay::default())?;
    let (_, shallow) = graph
        .node_by_commit_id(shallow_id)
        .expect("shallow commit is materialized");
    let missing_parent_id = repo
        .find_commit(shallow_id)?
        .parent_ids()
        .next()
        .expect("the shallow commit records its missing parent")
        .detach();

    assert!(shallow.parents().iter().any(|parent| matches!(
        graph.nodes()[*parent].kind(),
        NodeKind::Boundary {
            id,
            reason: BoundaryKind::Shallow,
        } if *id == missing_parent_id
    )));
    assert!(graph.node_by_commit_id(missing_parent_id).is_none());
    Ok(())
}
