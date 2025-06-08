//! Must be run as its own test as CWD is changed.

use but_db::DbHandle;

#[test]
fn open_with_relative_path() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("dir");
    std::fs::create_dir(&dir)?;

    std::env::set_current_dir(dir)?;

    let mut db = DbHandle::new_in_directory("..")?;
    assert!(
        db.hunk_assignments().list_all()?.is_empty(),
        "Relative paths don't work natively with Sqlite or Diesel, but we want it to work."
    );

    std::env::set_current_dir(tmp.path())?;
    let mut db = DbHandle::new_in_directory(".")?;
    assert!(
        db.hunk_assignments().list_all()?.is_empty(),
        "single-dot works as well (and there was a bug due to that previously)"
    );
    Ok(())
}
