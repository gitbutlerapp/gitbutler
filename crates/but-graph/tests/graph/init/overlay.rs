use but_graph::{Graph, NodeGraphEntrypoint, NodeKind, init::Overlay};
use gix::refs::Target;

use super::read_only_in_memory_scenario;

#[test]
fn overlay_adds_drops_and_moves_references() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let moved_id = repo.rev_parse_single("35ee481")?.detach();
    let superseded_id = repo.rev_parse_single("B")?.detach();
    let overlay = Overlay::default()
        .with_references([
            direct_ref("refs/heads/new-reference", moved_id)?,
            direct_ref("refs/heads/C", superseded_id)?,
        ])
        .with_references([direct_ref("refs/heads/C", moved_id)?])
        .with_dropped_references(["refs/heads/D".try_into()?]);
    let graph = Graph::from_repo(&repo, &*meta, Default::default(), overlay)?;

    assert_eq!(
        reference_target(&graph, "refs/heads/new-reference")?,
        moved_id
    );
    assert_eq!(reference_target(&graph, "refs/heads/C")?, moved_id);
    let dropped_name: gix::refs::FullName = "refs/heads/D".try_into()?;
    assert!(graph.node_by_ref_name(dropped_name.as_ref()).is_none());
    Ok(())
}

#[test]
fn overlay_entrypoint_preserves_symbolic_identity() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn")?;
    let id = write_root_commit(&repo)?;
    let name: gix::refs::FullName = "refs/heads/preview".try_into()?;
    let graph = Graph::from_repo(
        &repo,
        &*meta,
        Default::default(),
        Overlay::default().with_entrypoint(id, Some(name.clone())),
    )?;

    let NodeGraphEntrypoint::Node(index) = *graph.entrypoint() else {
        anyhow::bail!("symbolic overlay entrypoint must be materialized")
    };
    assert!(
        matches!(graph.nodes()[index].kind(), NodeKind::Commit { id: actual } if *actual == id)
    );
    assert_eq!(graph.entrypoint_ref(), Some(name.as_ref()));
    Ok(())
}

#[test]
fn overlay_entrypoint_rejects_a_stale_symbolic_reference() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let id = repo.rev_parse_single("A")?.detach();
    let name: gix::refs::FullName = "refs/heads/B".try_into()?;
    let err = Graph::from_repo(
        &repo,
        &*meta,
        Default::default(),
        Overlay::default().with_entrypoint(id, Some(name)),
    )
    .expect_err("the symbolic identity must agree with its discovered reference");

    assert!(err.to_string().contains("not entrypoint commit"));
    Ok(())
}

#[test]
fn dropping_head_ref_can_select_the_commit_directly() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("four-diamond")?;
    let head = repo.head_id()?.detach();
    let graph = Graph::from_repo(
        &repo,
        &*meta,
        Default::default(),
        Overlay::default()
            .with_entrypoint(head, None)
            .with_dropped_references(["refs/heads/merged".try_into()?]),
    )?;

    let NodeGraphEntrypoint::Node(index) = *graph.entrypoint() else {
        anyhow::bail!("detached overlay entrypoint must be materialized")
    };
    assert!(matches!(graph.nodes()[index].kind(), NodeKind::Commit { id } if *id == head));
    Ok(())
}

fn direct_ref(name: &str, id: gix::ObjectId) -> anyhow::Result<gix::refs::Reference> {
    Ok(gix::refs::Reference {
        name: name.try_into()?,
        target: Target::Object(id),
        peeled: Some(id),
    })
}

fn reference_target(graph: &Graph, name: &str) -> anyhow::Result<gix::ObjectId> {
    let name: gix::refs::FullName = name.try_into()?;
    graph
        .node_by_ref_name(name.as_ref())
        .and_then(|(_, reference)| reference.ref_info.commit_id)
        .ok_or_else(|| anyhow::anyhow!("missing reference {name}"))
}

fn write_root_commit(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let signature = gix::actor::Signature {
        name: "test".into(),
        email: "test@example.com".into(),
        time: gix::date::Time::new(0, 0),
    };
    Ok(repo
        .write_object(gix::objs::Commit {
            tree: repo.empty_tree().id,
            parents: Default::default(),
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: "root".into(),
            extra_headers: Vec::new(),
        })?
        .detach())
}
