use chrono::{DateTime, Utc};

use crate::{AppCacheHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    2026_01_19__15_00_00,
    "CREATE TABLE `update-check`(
    `id` INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
    `checked_at` TIMESTAMP NOT NULL,
    `up_to_date` BOOLEAN NOT NULL,
    `latest_version` TEXT NOT NULL,
    `release_notes` TEXT,
    `url` TEXT,
    `signature` TEXT,
    `suppressed_at` TIMESTAMP,
    `suppress_duration_hours` INTEGER
);",
)];

/// A utility for accessing the update checks.
pub struct UpdateCheckHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

/// A utility for mutating the update checks.
pub struct UpdateCheckHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl AppCacheHandle {
    /// Return a handle for read-only update checks.
    pub fn update_check(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: &self.conn }
    }

    /// Return a handle for mutating update checks.
    pub fn update_check_mut(&mut self) -> rusqlite::Result<UpdateCheckHandleMut<'_>> {
        Ok(UpdateCheckHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl Transaction<'_> {
    /// Return a handle for read-only update checks.
    pub fn update_check(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: self.inner() }
    }

    /// Return a handle for mutating update checks.
    pub fn update_check_mut(&mut self) -> rusqlite::Result<UpdateCheckHandleMut<'_>> {
        Ok(UpdateCheckHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

/// A cached update check result with timestamp.
///
/// This struct wraps the update check status with metadata about when the check occurred.
/// It's persisted to disk in the database to avoid redundant network requests.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedCheckResult {
    /// When this update check was performed (UTC timestamp).
    pub checked_at: DateTime<Utc>,
    /// The update status information from the server.
    pub status: CheckUpdateStatus,
    /// When the update notification was suppressed (UTC timestamp).
    /// This allows users to temporarily hide update notifications.
    pub suppressed_at: Option<DateTime<Utc>>,
    /// The number of hours to suppress update notifications.
    /// After this duration expires from `suppressed_at`, notifications will resume.
    pub suppress_duration_hours: Option<u32>,
}

/// Information about the latest available version and whether an update is needed.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckUpdateStatus {
    /// `true` if the current version matches the latest available version, `false` otherwise.
    ///
    /// When this is `false`, you should prompt the user to update or automatically download
    /// the update based on your application's update policy.
    pub up_to_date: bool,

    /// The version string of the latest available release (e.g., "0.18.3").
    ///
    /// This field is always present and can be compared with the current application version
    /// to determine if an update is available.
    pub latest_version: String,

    /// Markdown-formatted release notes describing changes in the latest version.
    ///
    /// This field is `None` if the server doesn't provide release notes.
    /// When present, this should be displayed to the user to inform them about
    /// what's new in the update.
    pub release_notes: Option<String>,

    /// Direct download URL for the update package.
    ///
    /// This field is `None` if no update is needed (`up_to_date == true`) or if the server
    /// doesn't provide a direct download link. The URL points to a platform-specific installer
    /// or update package.
    pub url: Option<String>,

    /// Cryptographic signature for verifying the authenticity of the downloaded update.
    ///
    /// This field is `None` if no signature is available. When present, this should be used
    /// to verify the integrity and authenticity of the downloaded update package before
    /// installation.
    pub signature: Option<String>,
}

impl UpdateCheckHandle<'_> {
    /// Retrieves the cached update check result if available.
    ///
    /// Returns `None` if no update check has been cached yet, or if there was an error.
    pub fn get(&self) -> Option<CachedCheckResult> {
        self.try_get().ok().flatten()
    }

    /// Like [`Self::get`], but fallible.
    pub fn try_get(&self) -> rusqlite::Result<Option<CachedCheckResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT checked_at, up_to_date, latest_version, release_notes, url, signature,
                    suppressed_at, suppress_duration_hours
             FROM `update-check` WHERE id = 1",
        )?;

        let mut rows = stmt.query([])?;

        match rows.next()? {
            Some(row) => {
                let checked_at_naive: chrono::NaiveDateTime = row.get(0)?;
                let checked_at = DateTime::from_naive_utc_and_offset(checked_at_naive, Utc);

                let suppressed_at_naive: Option<chrono::NaiveDateTime> = row.get(6)?;
                let suppressed_at = suppressed_at_naive
                    .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc));

                Ok(Some(CachedCheckResult {
                    checked_at,
                    status: CheckUpdateStatus {
                        up_to_date: row.get(1)?,
                        latest_version: row.get(2)?,
                        release_notes: row.get(3)?,
                        url: row.get(4)?,
                        signature: row.get(5)?,
                    },
                    suppressed_at,
                    suppress_duration_hours: row.get(7)?,
                }))
            }
            None => Ok(None),
        }
    }

    /// Returns the timestamp of the last update check if available.
    pub fn last_checked(&self) -> Option<DateTime<Utc>> {
        self.get().map(|cached| cached.checked_at)
    }
}

impl UpdateCheckHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: &self.sp }
    }

    /// Saves an update check result to the cache, without performing any checks.
    ///
    /// This replaces any existing cached result. Preserves suppression settings
    /// if they are still valid.
    pub fn save(self, result: &CachedCheckResult) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "INSERT OR REPLACE INTO `update-check`
             (id, checked_at, up_to_date, latest_version, release_notes, url, signature,
              suppressed_at, suppress_duration_hours)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                1, // Singleton record
                result.checked_at.naive_utc(),
                result.status.up_to_date,
                result.status.latest_version,
                result.status.release_notes,
                result.status.url,
                result.status.signature,
                result.suppressed_at.map(|dt| dt.naive_utc()),
                result.suppress_duration_hours,
            ],
        )?;

        sp.commit()?;
        Ok(())
    }

    /// Suppresses update notifications for a specified duration in `hours`.
    ///
    /// Updates the existing cache record with suppression settings.
    ///
    /// If no update check record exists yet, the function returns an errors.
    pub fn suppress(self, hours: u32) -> rusqlite::Result<()> {
        let sp = self.sp;

        // Check if a record exists
        let exists: bool = sp.query_row(
            "SELECT EXISTS(SELECT 1 FROM `update-check` WHERE id = 1)",
            [],
            |row| row.get(0),
        )?;

        if !exists {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::<
                dyn std::error::Error + Send + Sync,
            >::from(
                "No update check has been performed yet - cannot set suppression".to_string(),
            )));
        }

        // Update suppression fields
        sp.execute(
            "UPDATE `update-check`
             SET suppressed_at = ?1, suppress_duration_hours = ?2
             WHERE id = 1",
            rusqlite::params![Utc::now().naive_utc(), hours],
        )?;

        sp.commit()?;
        Ok(())
    }

    /// Clears suppression settings from the cache.
    pub fn clear_suppression(self) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "UPDATE `update-check`
             SET suppressed_at = NULL, suppress_duration_hours = NULL
             WHERE id = 1",
            [],
        )?;

        sp.commit()?;
        Ok(())
    }
}
