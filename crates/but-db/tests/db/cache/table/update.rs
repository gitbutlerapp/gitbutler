use but_db::cache::{CachedCheckResult, CheckUpdateStatus};
use chrono::DateTime;

use crate::cache::in_memory_cache;

#[test]
fn save_and_get() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    let result = sample_cached_result();

    // Save the result
    cache.update_check_mut()?.save(&result)?;

    // Retrieve it
    let retrieved = cache.update_check().try_get()?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();

    assert_eq!(retrieved.checked_at, result.checked_at);
    assert_eq!(retrieved.status, result.status);
    assert_eq!(retrieved.suppressed_at, None);
    assert_eq!(retrieved.suppress_duration_hours, None);

    Ok(())
}

#[test]
fn get_empty() -> anyhow::Result<()> {
    let cache = in_memory_cache();

    // No data saved yet
    let result = cache.update_check().try_get()?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn save_replaces_existing() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save first result
    let result1 = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: sample_status(true),
        suppressed_at: None,
        suppress_duration_hours: None,
    };
    cache.update_check_mut()?.save(&result1)?;

    // Save second result
    let result2 = CachedCheckResult {
        checked_at: DateTime::from_timestamp(2000000, 0).unwrap(),
        status: sample_status(false),
        suppressed_at: None,
        suppress_duration_hours: None,
    };
    cache.update_check_mut()?.save(&result2)?;

    // Should have the second result
    let retrieved = cache.update_check().try_get()?.unwrap();
    assert_eq!(retrieved.checked_at, result2.checked_at);
    assert!(!retrieved.status.up_to_date);

    Ok(())
}

#[test]
fn last_checked() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // No data yet
    assert!(cache.update_check().last_checked().is_none());

    // Save a result
    let result = sample_cached_result();
    cache.update_check_mut()?.save(&result)?;

    // Should return the checked_at time
    let last_checked = cache.update_check().last_checked();
    assert!(last_checked.is_some());
    assert_eq!(last_checked.unwrap(), result.checked_at);

    Ok(())
}

#[test]
fn suppress_update() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save a result first
    let result = sample_cached_result();
    cache.update_check_mut()?.save(&result)?;

    // Suppress for 24 hours
    cache.update_check_mut()?.suppress(24)?;

    // Should have suppression fields set
    let retrieved = cache.update_check().try_get()?.unwrap();
    assert!(retrieved.suppressed_at.is_some());
    assert_eq!(retrieved.suppress_duration_hours, Some(24));

    Ok(())
}

#[test]
fn suppress_update_fails_if_missing() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Try to suppress without anything saved.
    let err = cache.update_check_mut()?.suppress(24).unwrap_err();
    assert_eq!(
        err.to_string(),
        "No update check has been performed yet - cannot set suppression"
    );
    assert_eq!(
        cache.update_check().try_get()?,
        None,
        "Still nothing is there"
    );
    Ok(())
}

#[test]
fn clear_suppression() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save a result and suppress it
    let result = sample_cached_result();
    cache.update_check_mut()?.save(&result)?;
    cache.update_check_mut()?.suppress(24)?;

    // Verify suppression is set
    let retrieved = cache.update_check().try_get()?.unwrap();
    assert!(retrieved.suppressed_at.is_some());

    // Clear suppression
    cache.update_check_mut()?.clear_suppression()?;

    // Should have suppression cleared
    let retrieved = cache.update_check().try_get()?.unwrap();
    assert!(retrieved.suppressed_at.is_none());
    assert!(retrieved.suppress_duration_hours.is_none());

    Ok(())
}

#[test]
fn save_preserves_suppression() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    // Save a result with suppression
    let suppressed_at = DateTime::from_timestamp(1000000, 0).unwrap();
    let result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: sample_status(false),
        suppressed_at: Some(suppressed_at),
        suppress_duration_hours: Some(24),
    };
    cache.update_check_mut()?.save(&result)?;

    // Save a new result with suppression
    let new_result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(2000000, 0).unwrap(),
        status: sample_status(false),
        suppressed_at: Some(suppressed_at),
        suppress_duration_hours: Some(24),
    };
    cache.update_check_mut()?.save(&new_result)?;

    // Should preserve suppression settings
    let retrieved = cache.update_check().try_get()?.unwrap();
    assert_eq!(retrieved.suppressed_at, Some(suppressed_at));
    assert_eq!(retrieved.suppress_duration_hours, Some(24));

    Ok(())
}

#[test]
fn all_fields_persisted() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    let result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            up_to_date: false,
            latest_version: "2.0.0".to_string(),
            release_notes: Some("Major release".to_string()),
            url: Some("https://example.com/v2".to_string()),
            signature: Some("sig456".to_string()),
        },
        suppressed_at: Some(DateTime::from_timestamp(1100000, 0).unwrap()),
        suppress_duration_hours: Some(48),
    };

    cache.update_check_mut()?.save(&result)?;

    let retrieved = cache.update_check().try_get()?.unwrap();
    assert_eq!(retrieved, result);

    Ok(())
}

#[test]
fn optional_fields_can_be_none() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();

    let result = CachedCheckResult {
        checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
        status: CheckUpdateStatus {
            up_to_date: true,
            latest_version: "1.0.0".to_string(),
            release_notes: None,
            url: None,
            signature: None,
        },
        suppressed_at: None,
        suppress_duration_hours: None,
    };

    cache.update_check_mut()?.save(&result)?;

    let retrieved = cache.update_check().try_get()?.unwrap();
    assert_eq!(retrieved, result);
    assert!(retrieved.status.release_notes.is_none());
    assert!(retrieved.status.url.is_none());
    assert!(retrieved.status.signature.is_none());

    Ok(())
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
