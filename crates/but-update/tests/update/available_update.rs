use but_db::{
    AppCacheHandle,
    cache::{CachedCheckResult, CheckUpdateStatus},
};
use chrono::DateTime;

#[test]
fn returns_none_when_update_status_is_stale() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            // Note: The default for VERSION in `available_update` is 0.0.0. The test relies on this
            // for correctness. This is fragile but not easily circumvented at this time.
            valid_for_version: Some("0.0.1".to_string()),
            up_to_date: false,
            latest_version: "1.2.3".to_string(),
            release_notes: None,
            url: None,
            signature: None,
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&cached_result)?;

    let result = but_update::available_update(&cache)?;
    assert!(
        result.is_none(),
        "Expected stale update status to be filtered out"
    );

    Ok(())
}

#[test]
fn returns_update_when_versions_differ_and_valid_for_version_matches() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Seed the cache with up_to_date=false and a different version
    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            // Note: The default for VERSION in `available_update` is 0.0.0. The test relies on this
            // for correctness. This is fragile but not easily circumvented at this time.
            valid_for_version: Some("0.0.0".to_string()),
            up_to_date: false,
            latest_version: "1.2.3".to_string(),
            release_notes: Some("Bug fixes and improvements".to_string()),
            url: Some("https://example.com/download".to_string()),
            signature: Some("signature123".to_string()),
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&cached_result)?;

    // available_update should return Some because versions differ
    let result = but_update::available_update(&cache)?;
    assert!(
        result.is_some(),
        "Expected Some(AvailableUpdate) when versions differ"
    );

    let update = result.unwrap();
    assert_eq!(update.current_version, "0.0.0");
    assert_eq!(update.available_version, "1.2.3");
    assert_eq!(
        update.release_notes,
        Some("Bug fixes and improvements".to_string())
    );
    assert_eq!(update.url, Some("https://example.com/download".to_string()));

    Ok(())
}

#[test]
/// This is a forward compatibility test. Old data that lacks valid_for_version column should
/// operate under the assumption that the check is valid for whatever version is currently running.
fn returns_update_when_versions_differ_and_valid_for_version_is_none() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Seed the cache with up_to_date=false and a different version
    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            valid_for_version: None,
            up_to_date: false,
            latest_version: "1.2.3".to_string(),
            release_notes: Some("Bug fixes and improvements".to_string()),
            url: Some("https://example.com/download".to_string()),
            signature: Some("signature123".to_string()),
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&cached_result)?;

    // available_update should return Some because versions differ
    let result = but_update::available_update(&cache)?;
    assert!(
        result.is_some(),
        "Expected Some(AvailableUpdate) when versions differ"
    );

    let update = result.unwrap();
    assert_eq!(update.current_version, "0.0.0");
    assert_eq!(update.available_version, "1.2.3");
    assert_eq!(
        update.release_notes,
        Some("Bug fixes and improvements".to_string())
    );
    assert_eq!(update.url, Some("https://example.com/download".to_string()));

    Ok(())
}

fn in_memory_cache() -> AppCacheHandle {
    AppCacheHandle::new_at_path(":memory:")
}
