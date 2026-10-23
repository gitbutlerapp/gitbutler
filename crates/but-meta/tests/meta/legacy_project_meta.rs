//! Tests migration of legacy remote targets from `virtual_branches.toml` to local targets,
//! preserving stored commit IDs and push remotes without rewriting the legacy file.

use but_core::ref_metadata::{UnresolvedLegacyTargetReason, UnresolvedLegacyTargetRefError};
use but_meta::legacy_storage::read_legacy_project_meta;
use but_testsupport::{gix_testtools::tempfile::tempdir, read_only_in_memory_scenario};

fn repo_with_upstream() -> anyhow::Result<gix::Repository> {
    let mut repo = read_only_in_memory_scenario("dlib-standin")?;
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("branch.main.remote", "origin")?;
        config.set_raw_value("branch.main.merge", "refs/heads/trunk")?;
    }
    Ok(repo)
}

fn legacy_target(sha: gix::ObjectId) -> String {
    format!(
        "[default_target]\nremoteName = 'origin'\nbranchName = 'trunk'\nsha = '{sha}'\npushRemoteName = 'fork'\n"
    )
}

#[test]
fn resolves_local_target_and_preserves_commit_and_push_remote() -> anyhow::Result<()> {
    let repo = repo_with_upstream()?;
    let tmp = tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    let sha = repo.rev_parse_single(":/M1")?.detach();
    let content = legacy_target(sha);
    std::fs::write(&path, &content)?;

    let actual = read_legacy_project_meta(&path, &repo)?.expect("the file has a target");
    assert_eq!(
        actual.target_ref.map(|name| name.to_string()),
        Some("refs/heads/main".into()),
        "the configured upstream identifies the differently named local target"
    );
    assert_eq!(
        actual.target_commit_id,
        Some(sha),
        "migration preserves the stored cutoff"
    );
    assert_eq!(
        actual.push_remote.as_deref(),
        Some("fork"),
        "migration preserves the publishing remote"
    );
    assert_eq!(
        std::fs::read_to_string(path)?,
        content,
        "reading does not rewrite legacy metadata"
    );
    Ok(())
}

#[test]
fn missing_local_counterpart_errors() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("dlib-standin")?;
    let tmp = tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    let sha = repo.head_id()?.detach();
    std::fs::write(&path, legacy_target(sha))?;
    let err = read_legacy_project_meta(&path, &repo).unwrap_err();
    let unresolved = err
        .downcast_ref::<UnresolvedLegacyTargetRefError>()
        .unwrap_or_else(|| panic!("missing counterparts need an explicit choice: {err:#}"));
    assert_eq!(
        unresolved.reason,
        UnresolvedLegacyTargetReason::NoTrackingBranch,
        "no local branch has origin/trunk as its upstream"
    );
    assert_eq!(
        (
            unresolved.target_commit_id,
            unresolved.push_remote.as_deref()
        ),
        (Some(sha), Some("fork")),
        "porting keeps the stored cutoff and push remote until a target is chosen"
    );
    Ok(())
}

#[test]
fn ambiguous_local_counterparts_error() -> anyhow::Result<()> {
    let mut repo = repo_with_upstream()?;
    {
        let mut config = repo.config_snapshot_mut();
        config.set_raw_value("branch.confidence.remote", "origin")?;
        config.set_raw_value("branch.confidence.merge", "refs/heads/trunk")?;
    }
    let tmp = tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    std::fs::write(&path, legacy_target(repo.head_id()?.detach()))?;
    let err = read_legacy_project_meta(&path, &repo).unwrap_err();
    assert_eq!(
        err.downcast_ref::<UnresolvedLegacyTargetRefError>()
            .map(|err| err.reason),
        Some(UnresolvedLegacyTargetReason::MultipleTrackingBranches),
        "ambiguous counterparts must not be guessed: {err:#}"
    );
    Ok(())
}

#[test]
fn null_commit_id_reads_as_absent() -> anyhow::Result<()> {
    let repo = repo_with_upstream()?;
    let tmp = tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    std::fs::write(&path, legacy_target(repo.object_hash().null()))?;
    let actual = read_legacy_project_meta(&path, &repo)?.expect("the file has a target");
    assert_eq!(
        actual.target_commit_id, None,
        "the null ID is a placeholder, not a cutoff"
    );
    Ok(())
}

#[test]
fn missing_file_reads_as_absent() -> anyhow::Result<()> {
    let repo = repo_with_upstream()?;
    let tmp = tempdir()?;
    assert_eq!(
        read_legacy_project_meta(&tmp.path().join("missing.toml"), &repo)?,
        None,
        "repositories without legacy metadata need no migration"
    );
    Ok(())
}

#[test]
fn missing_target_reads_as_absent() -> anyhow::Result<()> {
    let repo = repo_with_upstream()?;
    let tmp = tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    std::fs::write(&path, "")?;
    assert_eq!(
        read_legacy_project_meta(&path, &repo)?,
        None,
        "a legacy file may have no default target"
    );
    Ok(())
}
