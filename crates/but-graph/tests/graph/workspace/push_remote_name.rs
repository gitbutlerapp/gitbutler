use but_core::RefMetadata;
use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

use super::target_meta;
use crate::init::utils::{add_workspace, read_only_in_memory_scenario, standard_options};

#[test]
fn with_local_target_uses_fetch_remote() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(ws.target_ref.is_some(), "fixture has a projected target");
    assert_eq!(
        ws.push_remote_name(&repo)?,
        Some("origin".into()),
        "the local target's fetch remote is origin"
    );

    Ok(())
}

#[test]
fn explicit_push_remote_overrides_target_fetch_remote() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_workspace(&mut meta);
    let mut ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    ws.graph.project_meta.push_remote = Some("fork".into());
    assert_eq!(
        ws.push_remote_name(&repo)?,
        Some("fork".into()),
        "feature branches use the explicit push remote instead of the target's remote"
    );
    Ok(())
}

#[test]
fn unconfigured_target_uses_projected_upstream_remote() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_workspace(&mut meta);
    let mut ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    ws.graph.project_meta.target_ref = None;
    ws.target_ref
        .as_mut()
        .expect("fixture has a target")
        .ref_name = "refs/remotes/origin/main".try_into()?;
    assert_eq!(
        ws.push_remote_name(&repo)?,
        Some("origin".into()),
        "the ad-hoc upstream fallback remains available without a configured target"
    );
    Ok(())
}

#[test]
fn configured_target_without_remote_does_not_use_projected_upstream() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_workspace(&mut meta);
    let mut ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;
    ws.graph.project_meta.target_ref = Some(but_core::ref_metadata::LocalRefName::new(
        "refs/heads/untracked".try_into()?,
    )?);
    ws.target_ref
        .as_mut()
        .expect("fixture has a target")
        .ref_name = "refs/remotes/origin/main".try_into()?;
    assert_eq!(
        ws.push_remote_name(&repo)?,
        None,
        "a configured local target must not inherit a projected fallback's remote"
    );
    Ok(())
}

#[test]
fn returns_none_when_no_target_and_no_push_remote() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(ws.target_ref.is_none(), "should not have a target_ref");
    assert!(
        ws.push_remote_name(&repo)?.is_none(),
        "should return None without target or metadata"
    );

    Ok(())
}

#[test]
fn target_local_tracking_ref_exists_when_other_branch_metadata_names_the_same_tip()
-> anyhow::Result<()> {
    let (repo, mut meta, _db) =
        read_only_in_memory_scenario("ws/no-ws-ref-no-ws-commit-two-branches")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bce0c5e (HEAD -> gitbutler/workspace, origin/main, main, B, A) M2
* 3183e43 M1

"#]]
    );

    add_workspace(&mut meta);
    // This is the state left by unapplying the last workspace stack: the branch
    // is no longer applied, but its branch metadata still disambiguates the
    // same commit that `main` and `origin/main` also point to.
    let branch_name = "refs/heads/A";
    let mut branch = meta.branch(branch_name.try_into()?)?;
    branch.update_times(false);
    meta.set_branch(&branch)?;

    let head_ref = repo.head_name()?;
    let ws = but_graph::nosegment::workspace(
        &repo,
        &*meta,
        but_graph::init::Overlay::default(),
        target_meta(&repo),
        head_ref.as_ref().map(|name| name.as_ref()),
        Some(repo.head_id()?.detach()),
        [],
        Vec::new(),
        false,
        None,
    )?;
    // the target remote and its local tracking branch get sibling links even when another branch owns the shared commit
    let local = ws
        .graph
        .segment_by_ref_name("refs/heads/main".try_into()?)
        .expect("main names a segment");
    let remote = ws
        .graph
        .segment_by_ref_name("refs/remotes/origin/main".try_into()?)
        .expect("origin/main names a segment");
    assert_eq!(
        local.remote_tracking_branch_segment_id,
        Some(remote.id),
        "main links to the segment of its remote-tracking branch even though A owns the shared commit"
    );
    assert_eq!(
        remote.sibling_segment_id,
        Some(local.id),
        "origin/main links back to the segment of main even though A owns the shared commit"
    );

    assert_eq!(
        ws.target_ref_name().map(|rn| rn.as_bstr()),
        Some("refs/heads/main".into()),
        "fixture should retain main as the configured local workspace target"
    );

    Ok(())
}
