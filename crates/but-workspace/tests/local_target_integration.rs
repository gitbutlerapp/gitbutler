//! Integration must apply the same upstream that supplies the incoming-commit counts.
use anyhow::Result;
use but_core::ref_metadata::{LocalRefName, ProjectMeta};
use but_testsupport::InMemoryRefMetadata;

#[test]
fn cutoff_matches_upstream_despite_lagging_local_target() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let tip = repo.rev_parse_single("origin/main")?.detach();
    let target = target_state(&repo, Some(tip))?;
    let upstream = target.upstream.expect("configured upstream");
    assert_eq!(upstream.tip, Some(tip), "the upstream tip is available");
    assert!(
        upstream.cutoff_matches_tip,
        "the stored cutoff matches upstream, not local main"
    );
    assert!(
        target.can_fast_forward_local_target,
        "pull can advance lagging local main even with a current cutoff"
    );
    Ok(())
}

#[test]
fn cutoff_trails_upstream() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let local = repo.rev_parse_single("main")?.detach();
    let target = target_state(&repo, Some(local))?;
    assert!(
        !target
            .upstream
            .expect("configured upstream")
            .cutoff_matches_tip,
        "matching local main does not mean the cutoff matches upstream"
    );
    Ok(())
}

#[test]
fn local_only_target_has_no_upstream() -> Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let local = repo.rev_parse_single("main")?.detach();
    let target = target_state(&repo, Some(local))?;
    assert!(
        target.upstream.is_none(),
        "a local target does not invent an upstream"
    );
    assert!(
        !target.can_fast_forward_local_target,
        "there is no upstream to advance to"
    );
    Ok(())
}

#[test]
fn missing_upstream_ref_retains_its_identity() -> Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.merge", "refs/heads/not-fetched")?;
    let local = repo.rev_parse_single("main")?.detach();
    let target = target_state(&repo, Some(local))?;
    let upstream = target
        .upstream
        .expect("configuration is independent of ref availability");
    assert_eq!(
        upstream.ref_name.full_name_bytes, "refs/remotes/origin/not-fetched",
        "the selected upstream must not fall back to local main"
    );
    assert!(upstream.tip.is_none(), "the configured ref is unavailable");
    assert!(
        !upstream.cutoff_matches_tip,
        "an unavailable tip cannot match the cutoff"
    );
    assert!(
        !target.can_fast_forward_local_target,
        "an unavailable tip cannot be pulled"
    );
    Ok(())
}

#[test]
fn unset_cutoff_does_not_match_upstream() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let target = target_state(&repo, None)?;
    assert!(
        !target
            .upstream
            .expect("configured upstream")
            .cutoff_matches_tip,
        "an inferred cutoff does not replace the missing stored cutoff"
    );
    Ok(())
}

#[test]
fn equal_local_target_does_not_need_fast_forward() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let tip = repo.rev_parse_single("origin/main")?.detach();
    repo.reference(
        "refs/heads/main",
        tip,
        gix::refs::transaction::PreviousValue::Any,
        "test: match upstream",
    )?;
    assert!(
        !target_state(&repo, Some(tip))?.can_fast_forward_local_target,
        "equal tips require no update"
    );
    Ok(())
}

#[test]
fn ahead_local_target_cannot_fast_forward() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let tip = repo.rev_parse_single("origin/main")?.detach();
    repo.reference(
        "refs/heads/main",
        tip,
        gix::refs::transaction::PreviousValue::Any,
        "test: match upstream before advancing",
    )?;
    repo.commit(
        "refs/heads/main",
        "local ahead",
        repo.find_commit(tip)?.tree_id()?,
        [tip],
    )?;
    assert!(
        !target_state(&repo, Some(tip))?.can_fast_forward_local_target,
        "pull must not rewind local commits"
    );
    Ok(())
}

#[test]
fn diverged_local_target_cannot_fast_forward() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let base = repo.rev_parse_single("main")?.detach();
    repo.commit(
        "refs/heads/main",
        "local divergence",
        repo.find_commit(base)?.tree_id()?,
        [base],
    )?;
    assert!(
        !target_state(&repo, Some(base))?.can_fast_forward_local_target,
        "pull must preserve local divergence"
    );
    Ok(())
}

#[test]
fn checked_out_local_target_cannot_fast_forward() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let local = repo.rev_parse_single("main")?.detach();
    let status = but_testsupport::git(&repo)
        .args(["checkout", "main"])
        .status()?;
    assert!(
        status.success(),
        "checkout prepares the worktree safety case"
    );
    assert!(
        !target_state(&repo, Some(local))?.can_fast_forward_local_target,
        "pull must not move a checked-out local target behind its worktree"
    );
    Ok(())
}

fn target_state(
    repo: &gix::Repository,
    cutoff: Option<gix::ObjectId>,
) -> Result<but_workspace::ui::ref_info::Target> {
    let project_meta = ProjectMeta {
        target_ref: Some(LocalRefName::new("refs/heads/main".try_into()?)?),
        target_commit_id: cutoff,
        ..Default::default()
    };
    let meta = InMemoryRefMetadata {
        workspaces: Vec::from([(but_core::WORKSPACE_REF_NAME.try_into()?, Default::default())]),
        ..Default::default()
    };
    let mut db = but_testsupport::project_db(repo)?;
    let workspace = but_graph::Graph::from_head(
        repo,
        &meta,
        project_meta,
        &mut db,
        but_graph::init::Options::limited(),
    )?
    .into_workspace()?;
    let info = but_workspace::graph_to_ref_info(&workspace, repo, Default::default())?;
    assert_eq!(
        workspace.graph.project_meta.target_commit_id, cutoff,
        "reading upstream state must not pin an inferred cutoff"
    );
    let ui: but_workspace::ui::RefInfo = info.try_into()?;
    Ok(ui.target.expect("local target is retained"))
}

/// Without an upstream the local target is the integration target, so moving it (as a local
/// `but merge` does) is what integration applies.
#[test]
fn integration_applies_a_local_target_without_an_upstream() -> Result<()> {
    // TODO(david): move this into one of the existing integration tests when they compile
    #[expect(unused_variables)]
    let todo_ = ();

    let (mut repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    repo.config_snapshot_mut()
        .remove_section("branch", Some("main".into()));
    let cutoff = repo.rev_parse_single("main")?.detach();
    let advanced = repo.rev_parse_single("origin/main")?.detach();
    repo.reference(
        "refs/heads/main",
        advanced,
        gix::refs::transaction::PreviousValue::Any,
        "test: advance the local target",
    )?;
    let project_meta = ProjectMeta {
        target_ref: Some(LocalRefName::new("refs/heads/main".try_into()?)?),
        target_commit_id: Some(cutoff),
        ..Default::default()
    };
    let mut meta = InMemoryRefMetadata {
        workspaces: Vec::from([(but_core::WORKSPACE_REF_NAME.try_into()?, Default::default())]),
        ..Default::default()
    };
    let mut db = but_testsupport::project_db(&repo)?;
    let mut workspace = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta.clone(),
        &mut db,
        but_graph::init::Options::limited(),
    )?
    .into_workspace()?;
    let outcome = but_workspace::integrate_upstream_with_hints(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        Vec::new(),
        &[],
        but_core::commit::SignCommit::IfSignCommitsEnabled,
    )?;
    assert_eq!(
        outcome.project_meta.target_commit_id,
        Some(advanced),
        "the cutoff advances to the local target"
    );
    outcome.rebase.materialize(Default::default())?;
    assert_eq!(
        repo.rev_parse_single("gitbutler/workspace^")?.detach(),
        advanced,
        "the workspace moves onto the local target"
    );
    assert_eq!(
        repo.rev_parse_single("main")?.detach(),
        advanced,
        "the local target itself is left where it is"
    );
    Ok(())
}

#[test]
fn integration_applies_upstream_when_local_target_lags() -> Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("integrate-local-target");
    let local = repo.rev_parse_single("main")?.detach();
    let upstream = repo.rev_parse_single("origin/main")?.detach();
    assert_ne!(local, upstream, "main must lag the fetched upstream");
    let project_meta = ProjectMeta {
        target_ref: Some(LocalRefName::new("refs/heads/main".try_into()?)?),
        target_commit_id: Some(local),
        ..Default::default()
    };
    let mut meta = InMemoryRefMetadata {
        workspaces: Vec::from([(but_core::WORKSPACE_REF_NAME.try_into()?, Default::default())]),
        ..Default::default()
    };
    let mut db = but_testsupport::project_db(&repo)?;
    let mut workspace = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta.clone(),
        &mut db,
        but_graph::init::Options::limited(),
    )?
    .into_workspace()?;
    assert_eq!(
        workspace
            .target_ref
            .as_ref()
            .map(|target| target.ref_name.as_bstr()),
        Some(gix::bstr::BStr::new("refs/heads/main")),
        "projection identifies the local target"
    );
    let outcome = but_workspace::integrate_upstream_with_hints(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        Vec::new(),
        &[],
        but_core::commit::SignCommit::IfSignCommitsEnabled,
    )?;
    assert_eq!(
        outcome.project_meta.target_commit_id,
        Some(upstream),
        "the returned cutoff advances to the advertised upstream tip"
    );
    let updated_meta = outcome.project_meta;
    outcome.rebase.materialize(Default::default())?;
    updated_meta.persist(&repo)?;
    assert_eq!(
        repo.rev_parse_single("gitbutler/workspace^")?.detach(),
        upstream,
        "integration actually applies the incoming upstream, not stale local main"
    );
    assert_eq!(
        ProjectMeta::resolve(&repo)?.target_commit_id,
        Some(upstream),
        "the stored cutoff catches up so the same update is not offered again"
    );
    Ok(())
}
