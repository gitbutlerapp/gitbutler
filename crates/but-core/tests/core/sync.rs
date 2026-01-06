use but_core::sync::{LockScope, try_exclusive_inter_process_access};
use but_testsupport::gix_testtools;

#[test]
fn exclusive_lock_prevents_second_acquisition() -> anyhow::Result<()> {
    let tmp = gix_testtools::tempfile::TempDir::new()?;

    // First process acquires the lock
    let _lock1 = try_exclusive_inter_process_access(tmp.path(), LockScope::AllOperations)?;

    // Second process should fail to acquire the same lock
    let result = try_exclusive_inter_process_access(tmp.path(), LockScope::AllOperations);
    assert!(
        result.is_err(),
        "Second lock acquisition should fail when first lock is held"
    );
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("already opened for writing by another GitButler instance"),
        "Error message should indicate lock is already held by another instance, got: {}",
        err_msg
    );

    Ok(())
}

#[test]
fn lock_released_on_drop() -> anyhow::Result<()> {
    let tmp = gix_testtools::tempfile::TempDir::new()?;

    // Acquire and immediately drop the lock
    {
        let _lock = try_exclusive_inter_process_access(tmp.path(), LockScope::AllOperations)?;
    } // lock dropped here

    // Should be able to acquire the lock again
    let _lock2 = try_exclusive_inter_process_access(tmp.path(), LockScope::AllOperations)?;

    Ok(())
}

#[test]
fn different_lock_scopes_use_different_lock_files() -> anyhow::Result<()> {
    let tmp = gix_testtools::tempfile::TempDir::new()?;

    // Acquire lock for all operations
    let _lock_all = try_exclusive_inter_process_access(tmp.path(), LockScope::AllOperations)?;

    // Should be able to acquire lock for background refresh operations (different lock file)
    let _lock_bg =
        try_exclusive_inter_process_access(tmp.path(), LockScope::BackgroundRefreshOperations)?;

    // Both locks should coexist
    assert!(
        tmp.path().join("project.lock").exists(),
        "AllOperations lock file should exist"
    );
    assert!(
        tmp.path().join("background-refresh.lock").exists(),
        "BackgroundRefreshOperations lock file should exist"
    );

    Ok(())
}

#[test]
fn background_refresh_lock_prevents_second_background_refresh() -> anyhow::Result<()> {
    let tmp = gix_testtools::tempfile::TempDir::new()?;

    // First process acquires background refresh lock
    let _lock1 =
        try_exclusive_inter_process_access(tmp.path(), LockScope::BackgroundRefreshOperations)?;

    // Second process should fail to acquire the same background refresh lock
    let result =
        try_exclusive_inter_process_access(tmp.path(), LockScope::BackgroundRefreshOperations);
    assert!(
        result.is_err(),
        "Second background refresh lock should fail when first is held"
    );
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("already being refreshed in the background by another GitButler instance"),
        "Error message should indicate background refresh is already in progress, got: {}",
        err_msg
    );

    Ok(())
}

#[test]
fn lock_scope_converts_to_correct_path() {
    let all_ops_path: std::path::PathBuf = LockScope::AllOperations.into();
    assert_eq!(all_ops_path, std::path::PathBuf::from("project.lock"));

    let bg_refresh_path: std::path::PathBuf = LockScope::BackgroundRefreshOperations.into();
    assert_eq!(
        bg_refresh_path,
        std::path::PathBuf::from("background-refresh.lock")
    );
}

#[test]
fn default_lock_scope_is_all_operations() {
    let default_scope = LockScope::default();
    assert!(matches!(default_scope, LockScope::AllOperations));
}
