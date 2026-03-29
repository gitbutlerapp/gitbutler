use but_core::sync::{LockScope, try_exclusive_inter_process_access};
use but_testsupport::gix_testtools;
use std::time::Duration;

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
        "Error message should indicate lock is already held by another instance, got: {err_msg}"
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
        "Error message should indicate background refresh is already in progress, got: {err_msg}"
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

/// Regression test: contention on one repo must not block access to an unrelated repo.
///
/// Before the fix, `shared_repo_access` / `exclusive_repo_access` held the global
/// `WORKTREE_LOCKS` mutex while blocking on the per-repo RwLock. This meant a thread
/// waiting for repo "A"'s RwLock would prevent any other thread from acquiring a lock
/// on repo "B", turning per-repo contention into a process-wide bottleneck.
#[test]
fn repo_lock_contention_does_not_block_unrelated_repos() {
    // Thread 1: hold exclusive access to repo "A".
    let exclusive_a = but_core::sync::exclusive_repo_access("/test/repo-a", None);

    // Thread 2: try to acquire shared access to repo "A".
    // This will block because thread 1 holds the exclusive lock.
    let (tx_ready, rx_ready) = std::sync::mpsc::channel();
    let blocked_thread = std::thread::spawn(move || {
        // Signal that we're about to enter the lock acquisition path.
        tx_ready.send(()).unwrap();
        // This blocks until exclusive_a is dropped.
        let _shared_a = but_core::sync::shared_repo_access("/test/repo-a");
    });

    // Wait until thread 2 has signalled it is about to call shared_repo_access.
    rx_ready.recv().unwrap();
    // Yield repeatedly so the OS scheduler runs thread 2 into the blocking
    // lock path. Unlike a fixed sleep this adapts to scheduler pressure and
    // avoids a hard-coded timing assumption.
    for _ in 0..100 {
        std::thread::yield_now();
    }

    // Thread 3 (this thread): access a completely unrelated repo "B".
    // With the bug, this would deadlock because thread 2 holds the global mutex
    // while waiting for repo A's RwLock.
    let (tx, rx) = std::sync::mpsc::channel();
    let probe = std::thread::spawn(move || {
        let _shared_b = but_core::sync::shared_repo_access("/test/repo-b");
        tx.send(()).ok();
    });

    // If repo "B" access completes within 2 seconds, the global mutex is not held
    // across RwLock acquisition — the bug is fixed.
    rx.recv_timeout(Duration::from_secs(2))
        .expect("accessing an unrelated repo should not be blocked by contention on another repo");
    probe.join().unwrap();

    // Release the exclusive lock so thread 2 can complete, then join to
    // surface any panics that occurred inside it.
    drop(exclusive_a);
    blocked_thread.join().unwrap();
}
