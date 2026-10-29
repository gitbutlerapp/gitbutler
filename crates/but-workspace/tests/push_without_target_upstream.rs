#[test]
fn feature_push_uses_publication_remote_without_target_upstream() -> anyhow::Result<()> {
    but_askpass::disable();
    let (mut repo, tmp) = but_testsupport::writable_scenario("push-single-branch");
    let remote_path = tmp.path().join("remote.git");
    let status = std::process::Command::new("git")
        .current_dir(repo.workdir().expect("fixture has a workdir"))
        .args(["remote", "set-url", "origin"])
        .arg(&remote_path)
        .status()?;
    assert!(status.success(), "fixture remote URL must be absolute");
    repo.config_snapshot_mut()
        .set_raw_value("branch.main.remote", "missing")?;
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: Some("origin".into()),
    };
    assert!(
        project_meta.remote_target_ref(&repo)?.is_none(),
        "main must have no configured target upstream"
    );
    let meta = but_testsupport::InMemoryRefMetadata::default();
    let mut db = but_testsupport::in_memory_db();
    let (info, workspace) = but_workspace::head_info_and_workspace(
        &repo,
        &meta,
        &mut db,
        but_workspace::ref_info::Options {
            project_meta,
            expensive_commit_info: true,
            ..Default::default()
        },
    )?;
    let branch: gix::refs::FullName = "refs/heads/top".try_into()?;
    but_workspace::legacy::workspace_branch_and_ancestors_push(
        &repo,
        &workspace,
        &info,
        &mut db,
        false,
        false,
        false,
        false,
        branch.as_ref(),
        false,
        false,
        Vec::new(),
    )?;
    let remote = gix::open(remote_path)?;
    assert_eq!(
        remote.rev_parse_single("refs/heads/top")?.detach(),
        repo.rev_parse_single("top")?.detach(),
        "the feature tip must reach the publication remote without a target upstream"
    );
    Ok(())
}
