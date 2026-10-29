use but_core::ref_metadata::{LocalRefName, ProjectMeta};

fn graph(repo: &gix::Repository) -> anyhow::Result<but_graph::Graph> {
    but_graph::Graph::from_head(
        repo,
        &but_testsupport::InMemoryRefMetadata::default(),
        ProjectMeta {
            target_ref: Some(LocalRefName::new("refs/heads/main".try_into()?)?),
            ..Default::default()
        },
        &mut but_testsupport::in_memory_db(),
        but_graph::init::Options::limited(),
    )
}

#[test]
fn missing_configured_upstream_does_not_select_local_tip() -> anyhow::Result<()> {
    let mut repo = but_testsupport::read_only_in_memory_scenario_named("cutoff-selection", "")?;
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.merge", "refs/heads/missing")?;
    let ws = graph(&repo)?.into_workspace()?;
    assert!(
        ws.target_commit.is_none(),
        "an unavailable upstream must not become the local tip"
    );
    Ok(())
}

#[test]
fn upstream_selection_does_not_require_a_segment_tracking_link() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario_named("cutoff-selection", "")?;
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    let mut graph = graph(&repo)?;
    let local_name: gix::refs::FullName = "refs/heads/main".try_into()?;
    let local = graph
        .segment_by_ref_name(local_name.as_ref())
        .expect("local target segment")
        .id;
    graph[local].remote_tracking_branch_segment_id = None;
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.target_commit.map(|target| target.commit_id),
        Some(upstream),
        "upstream identity must not depend on the segment tracking link"
    );
    Ok(())
}

#[test]
fn upstream_selection_does_not_require_a_named_local_segment() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario_named("cutoff-selection", "")?;
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    let mut graph = graph(&repo)?;
    let local_name: gix::refs::FullName = "refs/heads/main".try_into()?;
    let local = graph
        .segment_by_ref_name(local_name.as_ref())
        .expect("local target segment")
        .id;
    let local_ref = graph[local].ref_info.take().expect("named local segment");
    graph[local]
        .commits
        .first_mut()
        .expect("local target owns commits")
        .refs
        .push(local_ref);
    let ws = graph.into_workspace()?;
    assert_eq!(
        ws.target_commit.map(|target| target.commit_id),
        Some(upstream),
        "upstream selection must survive the local ref being carried by a commit"
    );
    Ok(())
}

#[test]
fn no_configured_upstream_selects_local_tip() -> anyhow::Result<()> {
    let mut repo = but_testsupport::read_only_in_memory_scenario_named("cutoff-selection", "")?;
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.remote", "missing")?;
    let local = repo.rev_parse_single("main")?.detach();
    let ws = graph(&repo)?.into_workspace()?;
    assert_eq!(
        ws.target_commit.map(|target| target.commit_id),
        Some(local),
        "local tip is the fallback only without a configured upstream"
    );
    Ok(())
}
