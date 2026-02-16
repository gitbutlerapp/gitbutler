use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, Transaction};

pub(crate) const M: &[M<'static>] = &[M::up(
    20260105095934,
    "CREATE TABLE `ci_checks`(
	`id` BIGINT NOT NULL PRIMARY KEY,
	`name` TEXT NOT NULL,
	`output_summary` TEXT NOT NULL,
	`output_text` TEXT NOT NULL,
	`output_title` TEXT NOT NULL,
	`started_at` TIMESTAMP,
	`status_type` TEXT NOT NULL,
	`status_conclusion` TEXT,
	`status_completed_at` TIMESTAMP,
	`head_sha` TEXT NOT NULL,
	`url` TEXT NOT NULL,
	`html_url` TEXT NOT NULL,
	`details_url` TEXT NOT NULL,
	`pull_requests` TEXT NOT NULL,
	`reference` TEXT NOT NULL,
	`last_sync_at` TIMESTAMP NOT NULL,
	`struct_version` INTEGER NOT NULL
);

CREATE INDEX `idx_ci_checks_reference` ON `ci_checks`(`reference`);",
)];

/// Tests are in `but-db/tests/db/table/ci_check.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CiCheck {
    pub id: i64,
    pub name: String,
    pub output_summary: String,
    pub output_text: String,
    pub output_title: String,
    pub started_at: Option<chrono::NaiveDateTime>,
    pub status_type: String,
    pub status_conclusion: Option<String>,
    pub status_completed_at: Option<chrono::NaiveDateTime>,
    pub head_sha: String,
    pub url: String,
    pub html_url: String,
    pub details_url: String,
    pub pull_requests: String,
    pub reference: String,
    pub last_sync_at: chrono::NaiveDateTime,
    pub struct_version: i32,
}

impl DbHandle {
    pub fn ci_checks(&self) -> CiChecksHandle<'_> {
        CiChecksHandle { conn: &self.conn }
    }

    pub fn ci_checks_mut(&mut self) -> rusqlite::Result<CiChecksHandleMut<'_>> {
        Ok(CiChecksHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    pub fn ci_checks(&self) -> CiChecksHandle<'_> {
        CiChecksHandle { conn: self.inner() }
    }

    pub fn ci_checks_mut(&mut self) -> rusqlite::Result<CiChecksHandleMut<'_>> {
        Ok(CiChecksHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

pub struct CiChecksHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct CiChecksHandleMut<'conn> {
    // Use savepoint as transaction, otherwise use `Connection`.
    sp: rusqlite::Savepoint<'conn>,
}

impl CiChecksHandle<'_> {
    /// Lists CI checks for a specific reference.
    pub fn list_for_reference(&self, ref_name: &str) -> rusqlite::Result<Vec<CiCheck>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, output_summary, output_text, output_title, started_at, 
                    status_type, status_conclusion, status_completed_at, head_sha, url, 
                    html_url, details_url, pull_requests, reference, last_sync_at, struct_version 
             FROM ci_checks WHERE reference = ?1",
        )?;

        let results = stmt.query_map([ref_name], |row| {
            Ok(CiCheck {
                id: row.get(0)?,
                name: row.get(1)?,
                output_summary: row.get(2)?,
                output_text: row.get(3)?,
                output_title: row.get(4)?,
                started_at: row.get(5)?,
                status_type: row.get(6)?,
                status_conclusion: row.get(7)?,
                status_completed_at: row.get(8)?,
                head_sha: row.get(9)?,
                url: row.get(10)?,
                html_url: row.get(11)?,
                details_url: row.get(12)?,
                pull_requests: row.get(13)?,
                reference: row.get(14)?,
                last_sync_at: row.get(15)?,
                struct_version: row.get(16)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }

    /// Lists all unique references that have CI checks in the database.
    // TODO: make this return `gix::refs::FullName`.
    pub fn list_all_references(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT reference FROM ci_checks")?;

        let results = stmt.query_map([], |row| row.get(0))?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl CiChecksHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> CiChecksHandle<'_> {
        CiChecksHandle { conn: &self.sp }
    }

    /// Sets the ci_checks table for a specific reference to the provided values.
    /// Any existing entries for this reference are deleted.
    ///
    /// Consumes this handle because it commits the internal savepoint/transaction,
    /// which is also consuming.
    pub fn set_for_reference(self, ref_name: &str, checks: Vec<CiCheck>) -> rusqlite::Result<()> {
        let sp = self.sp;

        // Delete existing entries for this reference
        sp.execute("DELETE FROM ci_checks WHERE reference = ?1", [ref_name])?;

        // Insert new entries
        if !checks.is_empty() {
            let mut stmt = sp.prepare(
                "INSERT INTO ci_checks (id, name, output_summary, output_text, output_title,
                                       started_at, status_type, status_conclusion, status_completed_at,
                                       head_sha, url, html_url, details_url, pull_requests,
                                       reference, last_sync_at, struct_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            )?;

            for check in checks {
                stmt.execute(rusqlite::params![
                    check.id,
                    check.name,
                    check.output_summary,
                    check.output_text,
                    check.output_title,
                    check.started_at,
                    check.status_type,
                    check.status_conclusion,
                    check.status_completed_at,
                    check.head_sha,
                    check.url,
                    check.html_url,
                    check.details_url,
                    check.pull_requests,
                    check.reference,
                    check.last_sync_at,
                    check.struct_version,
                ])?;
            }
        }

        sp.commit()?;
        Ok(())
    }

    /// Deletes all CI check entries for a specific reference.
    pub fn delete_for_reference(self, ref_name: &str) -> rusqlite::Result<()> {
        self.sp
            .execute("DELETE FROM ci_checks WHERE reference = ?1", [ref_name])?;
        self.sp.commit()?;
        Ok(())
    }
}
