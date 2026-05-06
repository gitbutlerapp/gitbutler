use but_db::{
    AppCacheHandle,
    cache::{CachedCheckResult, CheckUpdateStatus},
};
use chrono::DateTime;

#[test]
fn ignores_cached_update_without_build_version() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    let cached_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            up_to_date: false,
            latest_version: "1.2.3".to_string(),
            release_notes: Some("Test release notes".to_string()),
            url: Some("https://example.com/download".to_string()),
            signature: Some("signature123".to_string()),
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&cached_result)?;

    let result = but_update::available_update(&cache)?;
    assert!(
        result.is_none(),
        "Expected None when this binary was compiled without a release VERSION"
    );

    Ok(())
}

fn in_memory_cache() -> AppCacheHandle {
    AppCacheHandle::new_at_path(":memory:")
}
