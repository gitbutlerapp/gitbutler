use but_db::legacy::{VirtualBranches, legacy_to_snapshot, snapshot_to_legacy};

#[test]
fn reads_ignore_toml_and_writes_update_the_compatibility_mirror() -> anyhow::Result<()> {
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    let path = tmp.path().join("virtual_branches.toml");
    let mut db = but_db::DbHandle::new_in_directory(tmp.path())?;
    let stale = VirtualBranches {
        last_pushed_base: Some(gix::ObjectId::from_hex(
            b"1111111111111111111111111111111111111111",
        )?),
        ..Default::default()
    };
    std::fs::write(&path, toml::to_string(&stale)?)?;
    assert_eq!(
        snapshot_to_legacy(&db.virtual_branches().get_snapshot()?.unwrap_or_default())?
            .last_pushed_base,
        None,
        "legacy TOML is ignored even when no database state exists"
    );
    let state = VirtualBranches {
        last_pushed_base: Some(gix::ObjectId::from_hex(
            b"2222222222222222222222222222222222222222",
        )?),
        ..Default::default()
    };
    db.virtual_branches_mut()?
        .replace_snapshot(&legacy_to_snapshot(&state)?)?;
    assert_eq!(
        toml::from_str::<VirtualBranches>(&std::fs::read_to_string(&path)?)?,
        state,
        "database writes maintain a mirror for older versions"
    );
    std::fs::write(&path, toml::to_string(&stale)?)?;
    assert_eq!(
        snapshot_to_legacy(&db.virtual_branches().get_snapshot()?.unwrap_or_default())?,
        state,
        "newer TOML cannot overwrite database metadata"
    );
    std::fs::write(&path, "invalid TOML = [")?;
    assert_eq!(
        snapshot_to_legacy(&db.virtual_branches().get_snapshot()?.unwrap_or_default())?,
        state,
        "corrupt TOML cannot affect database metadata"
    );
    std::fs::remove_file(&path)?;
    assert_eq!(
        snapshot_to_legacy(&db.virtual_branches().get_snapshot()?.unwrap_or_default())?,
        state,
        "missing TOML cannot affect database metadata"
    );
    assert!(
        !path.exists(),
        "reads do not rewrite the compatibility mirror"
    );
    db.virtual_branches_mut()?
        .replace_snapshot(&legacy_to_snapshot(&state)?)?;
    assert_eq!(
        toml::from_str::<VirtualBranches>(&std::fs::read_to_string(&path)?)?,
        state,
        "writes regenerate a missing compatibility mirror"
    );
    Ok(())
}
