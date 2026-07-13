use but_graph::Graph;

use crate::init::utils::{
    add_workspace, add_workspace_with_target, read_only_in_memory_scenario, standard_options,
};

#[test]
fn distinguishes_target_base_from_ref_tip() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    let base_id = repo.rev_parse_single(":/M2")?.detach();
    let target_tip_id = repo.rev_parse_single("origin/main")?.detach();

    let project_meta = add_workspace_with_target(&mut meta, base_id);

    let ws = Graph::from_head(&repo, &*meta, project_meta, standard_options())?
        .validated()?
        .into_workspace()?;

    assert_eq!(ws.target_base_commit_id(), Some(base_id));
    assert_eq!(ws.target_ref_tip_commit_id(), Some(target_tip_id));
    assert_eq!(
        ws.legacy_target_ref_name().map(ToString::to_string),
        Some("refs/remotes/origin/main".to_string())
    );

    Ok(())
}

#[test]
fn target_helpers_return_none_without_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert_eq!(ws.target_base_commit_id(), None);
    assert_eq!(ws.target_ref_tip_commit_id(), None);
    assert_eq!(ws.legacy_target_ref_name(), None);

    Ok(())
}
