use but_db::{
    AppCacheHandle,
    cache::{CachedCheckResult, CheckUpdateStatus},
};
use chrono::DateTime;

#[test]
fn suppress_too_many_hours_fails() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save a result first
    cache.update_check_mut()?.save(&sample_cached_result())?;
    // Try to suppress for more than 720 hours (30 days)
    let result = but_update::suppress_update(&mut cache, 721);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Suppression duration cannot exceed 720 hours")
    );

    Ok(())
}

#[test]
fn suppress_zero_hours_fails() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save a result first
    cache.update_check_mut()?.save(&sample_cached_result())?;

    // Try to suppress for 0 hours
    let result = but_update::suppress_update(&mut cache, 0);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Suppression duration must be at least 1 hour")
    );

    Ok(())
}

fn in_memory_cache() -> AppCacheHandle {
    AppCacheHandle::new_at_path(":memory:")
}

fn sample_status(up_to_date: bool) -> CheckUpdateStatus {
    CheckUpdateStatus {
        up_to_date,
        latest_version: "1.2.3".to_string(),
        release_notes: Some("Bug fixes and improvements".to_string()),
        url: Some("https://example.com/download".to_string()),
        signature: Some("signature123".to_string()),
    }
}

fn sample_cached_result() -> CachedCheckResult {
    CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: sample_status(false),
        suppressed_at: None,
        suppress_duration_hours: None,
    }
}
