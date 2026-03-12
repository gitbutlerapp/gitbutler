use chrono::{DateTime, Duration};

use crate::cache::in_memory_cache;

#[test]
fn get_current_period_returns_zero_when_missing() -> anyhow::Result<()> {
    let cache = in_memory_cache();

    assert_eq!(cache.analytics().try_get_current_period()?, 0);
    assert_eq!(cache.analytics().get_current_period(), 0);

    Ok(())
}

#[test]
fn get_current_period_returns_zero_when_period_expired() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();
    let started_at = DateTime::from_timestamp(1_000_000, 0).unwrap();
    cache.analytics_mut()?.record_event_sent_at(started_at)?;

    let current_period = cache
        .analytics()
        .try_get_current_period_at(started_at + Duration::hours(24))?;

    assert_eq!(current_period, 0);
    Ok(())
}

#[test]
fn record_event_sent_initializes_period() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();
    let now = DateTime::from_timestamp(1_000_000, 0).unwrap();

    cache.analytics_mut()?.record_event_sent_at(now)?;

    let current_period = cache.analytics().try_get_current_period_at(now)?;
    assert_eq!(current_period, 1);

    Ok(())
}

#[test]
fn record_event_sent_increments_within_period() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();
    let started_at = DateTime::from_timestamp(1_000_000, 0).unwrap();

    cache.analytics_mut()?.record_event_sent_at(started_at)?;
    cache
        .analytics_mut()?
        .record_event_sent_at(started_at + Duration::hours(23))?;

    let current_period = cache
        .analytics()
        .try_get_current_period_at(started_at + Duration::hours(23))?;

    assert_eq!(current_period, 2);
    Ok(())
}

#[test]
fn record_event_sent_resets_after_period_expires() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();
    let started_at = DateTime::from_timestamp(1_000_000, 0).unwrap();

    cache.analytics_mut()?.record_event_sent_at(started_at)?;
    let next_period = started_at + Duration::hours(25);
    cache.analytics_mut()?.record_event_sent_at(next_period)?;

    let current_period = cache.analytics().try_get_current_period_at(next_period)?;

    assert_eq!(current_period, 1);
    Ok(())
}

#[test]
fn get_current_period_returns_zero_when_row_is_partial() -> anyhow::Result<()> {
    let tmp = tempfile::TempDir::new()?;
    let cache = but_db::AppCacheHandle::new_in_directory(Some(tmp.path()));
    let conn = rusqlite::Connection::open(tmp.path().join("app-cache.sqlite"))?;
    conn.execute(
        "INSERT INTO `analytics` (id, current_period_started_at, events_sent)
         VALUES (?1, NULL, NULL)",
        [1],
    )?;
    drop(conn);

    assert_eq!(cache.analytics().try_get_current_period()?, 0);
    Ok(())
}
