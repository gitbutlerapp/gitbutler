use chrono::{DateTime, Utc};

use crate::{AppCacheHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        2026_01_19__15_00_00,
        SchemaVersion::Zero,
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
    ),
    M::up(
        2026_05_18__08_00_00,
        SchemaVersion::Zero,
        "ALTER TABLE `update-check` ADD COLUMN `valid_for_version` TEXT NULL;",
    ),
];

// This table currently only has a single row representing the latest update check.
const SINGLETON_RECORD_ID: u8 = 1;

pub struct UpdateCheckHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct UpdateCheckHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl AppCacheHandle {
    pub fn update_check(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: &self.conn }
    }

    pub fn update_check_mut(&mut self) -> rusqlite::Result<UpdateCheckHandleMut<'_>> {
        Ok(UpdateCheckHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl Transaction<'_> {
    pub fn update_check(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: self.inner() }
    }

    pub fn update_check_mut(&mut self) -> rusqlite::Result<UpdateCheckHandleMut<'_>> {
        Ok(UpdateCheckHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

/// It's persisted to disk in the database to avoid redundant network requests.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedCheckResult {
    pub checked_at: DateTime<Utc>,
    pub status: CheckUpdateStatus,
    /// This allows users to temporarily hide update notifications.
    pub suppressed_at: Option<DateTime<Utc>>,
    /// After this duration expires from `suppressed_at`, notifications will resume.
    pub suppress_duration_hours: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckUpdateStatus {
    /// This is used to check for cache staleness. As the version comparison is done server side,
    /// the value of [`Self::up_to_date`] is only valid if the current version of the app matches
    /// the version running at the time of the update check.
    ///
    /// # Compatibility
    /// This is nullable only for forward compatibility reasons, as this column hasn't always
    /// existed. Removing nullability requires bumping the schema version as old clients will no
    /// longer be able to write to the table.
    pub valid_for_version: Option<String>,

    pub up_to_date: bool,

    /// The version string of the latest available release (e.g., "0.18.3").
    pub latest_version: String,

    /// Markdown-formatted release notes describing changes in the latest version.
    ///
    /// This field is `None` if the server doesn't provide release notes.
    /// When present, this should be displayed to the user to inform them about
    /// what's new in the update.
    pub release_notes: Option<String>,

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
    /// Returns `None` if no update check has been cached yet, or if there was an error.
    pub fn get(&self) -> Option<CachedCheckResult> {
        self.try_get().ok().flatten()
    }

    pub fn try_get(&self) -> rusqlite::Result<Option<CachedCheckResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT checked_at, up_to_date, latest_version, release_notes, url, signature, valid_for_version,
                    suppressed_at, suppress_duration_hours
             FROM `update-check` WHERE id = 1",
        )?;

        let mut rows = stmt.query([])?;

        match rows.next()? {
            Some(row) => {
                let checked_at_naive: chrono::NaiveDateTime = row.get(0)?;
                let checked_at = DateTime::from_naive_utc_and_offset(checked_at_naive, Utc);

                let suppressed_at_naive: Option<chrono::NaiveDateTime> = row.get(7)?;
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
                        valid_for_version: row.get(6)?,
                    },
                    suppressed_at,
                    suppress_duration_hours: row.get(8)?,
                }))
            }
            None => Ok(None),
        }
    }

    pub fn last_checked(&self) -> Option<DateTime<Utc>> {
        self.get().map(|cached| cached.checked_at)
    }
}

impl UpdateCheckHandleMut<'_> {
    pub fn to_ref(&self) -> UpdateCheckHandle<'_> {
        UpdateCheckHandle { conn: &self.sp }
    }

    /// This replaces any existing cached result.
    pub fn save(self, result: &CachedCheckResult) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "INSERT OR REPLACE INTO `update-check`
             (id, checked_at, up_to_date, latest_version, release_notes, url, signature, valid_for_version,
              suppressed_at, suppress_duration_hours)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                SINGLETON_RECORD_ID,
                result.checked_at.naive_utc(),
                result.status.up_to_date,
                result.status.latest_version,
                result.status.release_notes,
                result.status.url,
                result.status.signature,
                result.status.valid_for_version,
                result.suppressed_at.map(|dt| dt.naive_utc()),
                result.suppress_duration_hours,
            ],
        )?;

        sp.commit()?;
        Ok(())
    }

    /// Deletes the update check, which is equivalent to cache invalidation.
    ///
    /// This operation is idempotent and is safe to perform regardless of if an update check record
    /// exists or not.
    pub fn delete(self) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "DELETE FROM `update-check` WHERE id = ?1;",
            rusqlite::params![SINGLETON_RECORD_ID],
        )?;
        sp.commit()?;

        Ok(())
    }

    /// If no update check record exists yet, the function returns an error.
    pub fn suppress(self, hours: u32) -> rusqlite::Result<()> {
        let sp = self.sp;

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

        sp.execute(
            "UPDATE `update-check`
             SET suppressed_at = ?1, suppress_duration_hours = ?2
             WHERE id = ?3",
            rusqlite::params![Utc::now().naive_utc(), hours, SINGLETON_RECORD_ID],
        )?;

        sp.commit()?;
        Ok(())
    }

    pub fn clear_suppression(self) -> rusqlite::Result<()> {
        let sp = self.sp;

        sp.execute(
            "UPDATE `update-check`
             SET suppressed_at = NULL, suppress_duration_hours = NULL
             WHERE id = ?1",
            [SINGLETON_RECORD_ID],
        )?;

        sp.commit()?;
        Ok(())
    }
}
