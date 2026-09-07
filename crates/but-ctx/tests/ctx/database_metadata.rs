use but_ctx::Context;
use but_testsupport::gix_testtools::tempfile::TempDir;

#[test]
fn metadata_access_does_not_create_live_toml() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let ctx = Context::from_repo_for_testing(gix::init(tmp.path())?)?;
    let path = ctx.project_data_dir().join("virtual_branches.toml");
    let meta = ctx.db.get_cache()?.meta()?;
    let workspace = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?);
    assert!(workspace.is_none(), "a new database has no stacks");
    drop(meta);
    assert!(
        !path.exists(),
        "metadata access must never create live TOML"
    );
    let mut db = ctx.db.get_cache_mut()?;
    db.meta_mut()?
        .set_branch("refs/heads/new-branch".try_into()?, &Default::default())?;
    assert!(
        !path.exists(),
        "metadata writes must never create live TOML"
    );
    Ok(())
}

#[test]
fn uninitialized_database_does_not_import_live_toml() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let ctx = Context::from_repo_for_testing(gix::init(tmp.path())?)?;
    std::fs::create_dir_all(ctx.project_data_dir())?;
    let path = ctx.project_data_dir().join("virtual_branches.toml");
    let original = include_str!("../../../but-meta/tests/fixtures/legacy/virtual-branches-01.toml");
    std::fs::write(&path, original)?;

    let meta = ctx.db.get_cache()?.meta()?;
    let workspace = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?);
    assert!(workspace.is_none(), "TOML must not seed database metadata");
    drop(meta);
    assert_eq!(
        std::fs::read_to_string(path)?,
        original,
        "live TOML is untouched"
    );
    Ok(())
}

#[test]
fn database_metadata_ignores_changed_live_toml() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let ctx = Context::from_repo_for_testing(gix::init(tmp.path())?)?;
    let expected = but_db::VirtualBranchesSnapshot {
        state: but_db::VbState {
            initialized: true,
            ..Default::default()
        },
        ..Default::default()
    };
    ctx.db
        .get_cache_mut()?
        .virtual_branches_mut()?
        .replace_snapshot(&expected)?;
    let path = ctx.project_data_dir().join("virtual_branches.toml");
    for original in [
        "this is not valid TOML [",
        include_str!("../../../but-meta/tests/fixtures/legacy/virtual-branches-01.toml"),
    ] {
        std::fs::write(&path, original)?;
        drop(ctx.db.get_cache()?.meta()?);
        assert_eq!(
            std::fs::read_to_string(&path)?,
            original,
            "metadata reads do not repair TOML"
        );
        assert_eq!(
            ctx.db.get_cache()?.virtual_branches().get_snapshot()?,
            Some(expected.clone()),
            "TOML cannot change database state"
        );
    }
    Ok(())
}
