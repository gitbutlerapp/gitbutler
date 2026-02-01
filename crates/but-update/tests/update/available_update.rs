use but_db::{
    AppCacheHandle,
    cache::{CachedCheckResult, CheckUpdateStatus},
};
use chrono::DateTime;

#[test]
fn no_op_update_when_versions_match() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Seed the cache with up_to_date=false but latest_version="0.0.0"
    // which matches the VERSION fallback in available_update()
    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            up_to_date: false,
            latest_version: "0.0.0".to_string(),
            release_notes: Some("Test release notes".to_string()),
            url: Some("https://example.com/download".to_string()),
            signature: Some("signature123".to_string()),
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&cached_result)?;

    // available_update should return None because the versions match (no-op)
    let result = but_update::available_update(&cache)?;
    assert!(
        result.is_none(),
        "Expected None for no-op update where latest_version matches current_version"
    );

    Ok(())
}

#[test]
fn returns_update_when_versions_differ() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Seed the cache with up_to_date=false and a different version
    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
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
    assert!(result.is_some(), "Expected Some(AvailableUpdate) when versions differ");

    let update = result.unwrap();
    assert_eq!(update.current_version, "0.0.0");
    assert_eq!(update.available_version, "1.2.3");
    assert_eq!(update.release_notes, Some("Bug fixes and improvements".to_string()));
    assert_eq!(update.url, Some("https://example.com/download".to_string()));

    Ok(())
}

fn in_memory_cache() -> AppCacheHandle {
    AppCacheHandle::new_at_path(":memory:")
}
