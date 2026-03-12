use chrono::{DateTime, Duration, NaiveDateTime, Utc};

use crate::{AppCacheHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    2026_03_12__12_00_00,
    SchemaVersion::Zero,
    "CREATE TABLE `analytics`(
    `id` INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
    `current_period_started_at` TIMESTAMP,
    `events_sent` INTEGER
);",
)];

/// The singleton row id for the analytics cache table.
const SINGLETON_RECORD_ID: u8 = 1;
/// The duration of the analytics counting period.
const ANALYTICS_PERIOD_DURATION: Duration = Duration::hours(24);

/// Read-only access to the analytics cache table.
pub struct AnalyticsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// Mutating access to the analytics cache table.
pub struct AnalyticsHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl AppCacheHandle {
    /// Return a handle for read-only analytics cache access.
    pub fn analytics(&self) -> AnalyticsHandle<'_> {
        AnalyticsHandle { conn: &self.conn }
    }

    /// Return a handle for mutating analytics cache access.
    pub fn analytics_mut(&mut self) -> rusqlite::Result<AnalyticsHandleMut<'_>> {
        Ok(AnalyticsHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl Transaction<'_> {
    /// Return a handle for read-only analytics cache access.
    pub fn analytics(&self) -> AnalyticsHandle<'_> {
        AnalyticsHandle { conn: self.inner() }
    }

    /// Return a handle for mutating analytics cache access.
    pub fn analytics_mut(&mut self) -> rusqlite::Result<AnalyticsHandleMut<'_>> {
        Ok(AnalyticsHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

impl AnalyticsHandle<'_> {
    /// Return the number of events sent in the current 24-hour period.
    ///
    /// If the period has not started yet, the stored row is incomplete, or the
    /// recorded period is older than 24 hours, this returns `0`.
    pub fn get_current_period(&self) -> u64 {
        self.try_get_current_period().unwrap_or(0)
    }

    /// Like [`Self::get_current_period`], but fallible.
    pub fn try_get_current_period(&self) -> rusqlite::Result<u64> {
        self.try_get_current_period_at(Utc::now())
    }

    /// Like [`Self::try_get_current_period`], but evaluates the period at `now`.
    pub fn try_get_current_period_at(&self, now: DateTime<Utc>) -> rusqlite::Result<u64> {
        let mut stmt = self.conn.prepare(
            "SELECT current_period_started_at, events_sent
             FROM `analytics`
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query([SINGLETON_RECORD_ID])?;

        let Some(row) = rows.next()? else {
            return Ok(0);
        };

        let current_period_started_at: Option<NaiveDateTime> = row.get(0)?;
        let events_sent: Option<i64> = row.get(1)?;
        Ok(current_period_value(
            current_period_started_at,
            events_sent,
            now,
        ))
    }
}

impl AnalyticsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> AnalyticsHandle<'_> {
        AnalyticsHandle { conn: &self.sp }
    }

    /// Record one sent event in the current 24-hour period.
    ///
    /// If there is no usable singleton row yet, or the stored period is older
    /// than 24 hours, the period is reset to `now` and the count becomes `1`.
    pub fn record_event_sent(self) -> rusqlite::Result<()> {
        self.record_event_sent_at(Utc::now())
    }

    /// Like [`Self::record_event_sent`], but uses the provided timestamp.
    pub fn record_event_sent_at(self, now: DateTime<Utc>) -> rusqlite::Result<()> {
        let sp = self.sp;
        let current_count = AnalyticsHandle { conn: &sp }.try_get_current_period_at(now)?;

        if current_count == 0 {
            sp.execute(
                "INSERT INTO `analytics` (id, current_period_started_at, events_sent)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    current_period_started_at = excluded.current_period_started_at,
                    events_sent = excluded.events_sent",
                rusqlite::params![SINGLETON_RECORD_ID, now.naive_utc(), 1_i64],
            )?;
        } else {
            let next_count = i64::try_from(current_count + 1).unwrap_or(i64::MAX);
            sp.execute(
                "UPDATE `analytics`
                 SET events_sent = ?1
                 WHERE id = ?2",
                rusqlite::params![next_count, SINGLETON_RECORD_ID],
            )?;
        }

        sp.commit()?;
        Ok(())
    }
}

/// Compute the current period event count for the stored values at `now`.
fn current_period_value(
    current_period_started_at: Option<NaiveDateTime>,
    events_sent: Option<i64>,
    now: DateTime<Utc>,
) -> u64 {
    let Some(current_period_started_at) = current_period_started_at else {
        return 0;
    };
    let Some(events_sent) = events_sent else {
        return 0;
    };
    let current_period_started_at =
        DateTime::from_naive_utc_and_offset(current_period_started_at, Utc);

    if now - current_period_started_at >= ANALYTICS_PERIOD_DURATION {
        return 0;
    }

    u64::try_from(events_sent).unwrap_or(0)
}
