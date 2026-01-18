use std::env;
use std::fs;
use std::sync::Mutex;

use but_update::{CheckUpdateStatus, last_checked};
use chrono::Utc;

// Serial test execution lock to prevent environment variable conflicts
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Helper to ensure we have a clean test environment
fn setup_test_cache_dir() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    // SAFETY: This is safe in tests as we control the test environment
    // and tests are not expected to be run in parallel with shared state.
    unsafe {
        env::set_var("E2E_TEST_APP_DATA_DIR", temp_dir.path());
    }
    temp_dir
}

#[test]
fn test_last_checked_returns_none_when_no_cache_exists() {
    let _lock = TEST_LOCK.lock().unwrap();
    let _temp_dir = setup_test_cache_dir();

    let result = last_checked().expect("Should not error");
    assert!(result.is_none(), "Expected None when no cache file exists");
}

#[test]
fn test_cache_persists_after_check_status() {
    let _lock = TEST_LOCK.lock().unwrap();
    let _temp_dir = setup_test_cache_dir();

    // Create a mock status
    let status = CheckUpdateStatus {
        up_to_date: false,
        latest_version: "1.0.0".to_string(),
        release_notes: Some("Test release notes".to_string()),
        url: Some("https://example.com/download".to_string()),
        signature: None,
    };

    // Manually save (simulating what check_status does)
    but_update::cache::save(&status);

    // Verify it was cached
    let last_check = last_checked()
        .expect("Should not error")
        .expect("Should have a cached timestamp");

    // Check that the timestamp is recent (within last 5 seconds)
    let elapsed = Utc::now().signed_duration_since(last_check);
    assert!(
        elapsed.num_seconds() < 5,
        "Cached timestamp should be recent, but was {} seconds ago",
        elapsed.num_seconds()
    );
}

#[test]
fn test_cache_file_location() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    let status = CheckUpdateStatus {
        up_to_date: true,
        latest_version: "0.5.0".to_string(),
        release_notes: None,
        url: None,
        signature: None,
    };

    but_update::cache::save(&status);

    // Verify the cache file was created in the expected location
    let cache_file = temp_dir.path().join("cache").join("update-check.json");
    assert!(
        cache_file.exists(),
        "Cache file should exist at {:?}",
        cache_file
    );

    // Verify it contains valid JSON
    let contents = fs::read_to_string(&cache_file).expect("Should read cache file");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("Should parse as JSON");

    assert!(parsed.get("checked_at").is_some(), "Should have checked_at");
    assert!(parsed.get("status").is_some(), "Should have status");
}

#[test]
fn test_cache_survives_multiple_calls() {
    let _lock = TEST_LOCK.lock().unwrap();
    let _temp_dir = setup_test_cache_dir();

    let status1 = CheckUpdateStatus {
        up_to_date: false,
        latest_version: "1.0.0".to_string(),
        release_notes: None,
        url: None,
        signature: None,
    };

    but_update::cache::save(&status1);
    let first_check = last_checked()
        .expect("Should not error")
        .expect("Should have first timestamp");

    // Small delay to ensure timestamps differ
    std::thread::sleep(std::time::Duration::from_millis(100));

    let status2 = CheckUpdateStatus {
        up_to_date: true,
        latest_version: "2.0.0".to_string(),
        release_notes: Some("Updated".to_string()),
        url: None,
        signature: None,
    };

    but_update::cache::save(&status2);
    let second_check = last_checked()
        .expect("Should not error")
        .expect("Should have second timestamp");

    // Second timestamp should be after first
    assert!(
        second_check > first_check,
        "Second check should be after first"
    );
}

#[test]
fn test_corrupted_cache_returns_none() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    // Create the cache directory and write invalid JSON
    let cache_dir = temp_dir.path().join("cache");
    fs::create_dir_all(&cache_dir).expect("Should create cache dir");

    let cache_file = cache_dir.join("update-check.json");
    fs::write(&cache_file, "invalid json content").expect("Should write invalid content");

    // Should return None instead of erroring
    let result = last_checked().expect("Should not error on corrupted cache");
    assert!(
        result.is_none(),
        "Should return None when cache is corrupted"
    );
}

#[test]
fn test_update_check_lock_prevents_concurrent_access() {
    let _lock = TEST_LOCK.lock().unwrap();
    let _temp_dir = setup_test_cache_dir();

    // Acquire the lock
    let lock1 = but_update::try_update_check_lock().expect("Should acquire first lock");

    // Try to acquire it again - should fail
    let lock2_result = but_update::try_update_check_lock();
    assert!(
        lock2_result.is_err(),
        "Second lock attempt should fail while first is held"
    );

    // Drop the first lock
    drop(lock1);

    // Now we should be able to acquire it
    let lock3 =
        but_update::try_update_check_lock().expect("Should acquire lock after first is dropped");
    drop(lock3);
}

#[test]
fn test_update_check_lock_location() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    // Acquire the lock
    let _lock_guard = but_update::try_update_check_lock().expect("Should acquire lock");

    // Verify the lock file was created in the expected location
    let lock_file = temp_dir.path().join("cache").join("update-check.lock");
    assert!(
        lock_file.exists(),
        "Lock file should exist at {:?}",
        lock_file
    );
}

#[test]
fn test_suppression_fields_are_preserved() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    // Create initial cache with suppression
    let cache_file = temp_dir.path().join("cache").join("update-check.json");
    fs::create_dir_all(cache_file.parent().unwrap()).expect("Should create cache dir");

    let suppressed_at = Utc::now();
    let initial_cache = serde_json::json!({
        "checked_at": suppressed_at.to_rfc3339(),
        "status": {
            "up_to_date": false,
            "latest_version": "1.0.0"
        },
        "suppressed_at": suppressed_at.to_rfc3339(),
        "suppress_duration_hours": 168
    });

    fs::write(&cache_file, serde_json::to_string_pretty(&initial_cache).unwrap())
        .expect("Should write initial cache");

    // Save new status (should preserve suppression)
    let status = CheckUpdateStatus {
        up_to_date: false,
        latest_version: "1.0.1".to_string(),
        release_notes: None,
        url: None,
        signature: None,
    };

    but_update::cache::save(&status);

    // Read back the cache and verify suppression is preserved
    let contents = fs::read_to_string(&cache_file).expect("Should read cache file");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("Should parse as JSON");

    assert!(
        parsed.get("suppressed_at").is_some(),
        "suppressed_at should be preserved"
    );
    assert_eq!(
        parsed.get("suppress_duration_hours").and_then(|v| v.as_u64()),
        Some(168),
        "suppress_duration_hours should be preserved"
    );
}

#[test]
fn test_expired_suppression_is_cleared() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    // Create cache with expired suppression (suppressed 240 hours ago, duration was 168 hours)
    let cache_file = temp_dir.path().join("cache").join("update-check.json");
    fs::create_dir_all(cache_file.parent().unwrap()).expect("Should create cache dir");

    let suppressed_at = Utc::now() - chrono::Duration::hours(240);
    let initial_cache = serde_json::json!({
        "checked_at": Utc::now().to_rfc3339(),
        "status": {
            "up_to_date": false,
            "latest_version": "1.0.0"
        },
        "suppressed_at": suppressed_at.to_rfc3339(),
        "suppress_duration_hours": 168
    });

    fs::write(&cache_file, serde_json::to_string_pretty(&initial_cache).unwrap())
        .expect("Should write initial cache");

    // Save new status (should clear expired suppression)
    let status = CheckUpdateStatus {
        up_to_date: false,
        latest_version: "1.0.1".to_string(),
        release_notes: None,
        url: None,
        signature: None,
    };

    but_update::cache::save(&status);

    // Read back the cache and verify suppression is cleared
    let contents = fs::read_to_string(&cache_file).expect("Should read cache file");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("Should parse as JSON");

    assert!(
        parsed.get("suppressed_at").is_none(),
        "suppressed_at should be cleared when expired"
    );
    assert!(
        parsed.get("suppress_duration_hours").is_none(),
        "suppress_duration_hours should be cleared when expired"
    );
}

#[test]
fn test_suppression_on_edge_case_exactly_expired() {
    let _lock = TEST_LOCK.lock().unwrap();
    let temp_dir = setup_test_cache_dir();

    // Create cache where suppression expires exactly now
    let cache_file = temp_dir.path().join("cache").join("update-check.json");
    fs::create_dir_all(cache_file.parent().unwrap()).expect("Should create cache dir");

    let suppressed_at = Utc::now() - chrono::Duration::hours(168);
    let initial_cache = serde_json::json!({
        "checked_at": Utc::now().to_rfc3339(),
        "status": {
            "up_to_date": false,
            "latest_version": "1.0.0"
        },
        "suppressed_at": suppressed_at.to_rfc3339(),
        "suppress_duration_hours": 168
    });

    fs::write(&cache_file, serde_json::to_string_pretty(&initial_cache).unwrap())
        .expect("Should write initial cache");

    // Save new status
    let status = CheckUpdateStatus {
        up_to_date: false,
        latest_version: "1.0.1".to_string(),
        release_notes: None,
        url: None,
        signature: None,
    };

    but_update::cache::save(&status);

    // Read back the cache
    let contents = fs::read_to_string(&cache_file).expect("Should read cache file");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("Should parse as JSON");

    // Due to timing, this should be cleared (now > suppress_until)
    assert!(
        parsed.get("suppressed_at").is_none(),
        "suppressed_at should be cleared when exactly at expiration boundary"
    );
}
