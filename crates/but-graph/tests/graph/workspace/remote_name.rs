use but_graph::Graph;

use crate::init::utils::{add_workspace, add_workspace_without_target, read_only_in_memory_scenario, standard_options};

#[test]
fn with_target_ref_extracts_remote_name() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(ws.target_ref.is_some());
    assert_eq!(
        ws.remote_name(),
        Some("origin".into()),
        "target_ref is 'refs/remotes/origin/main', should extract 'origin'"
    );

    Ok(())
}

#[test]
fn returns_none_when_no_target_and_no_push_remote() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace_without_target(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(ws.target_ref.is_none(), "should not have a target_ref");
    assert!(
        ws.remote_name().is_none(),
        "should return None without target or metadata"
    );

    Ok(())
}
