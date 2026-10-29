#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::{DbHandle, M, SchemaVersion, Transaction};

pub(crate) const M: &[M<'static>] = &[
    M::up(
        20260101223932,
        SchemaVersion::Zero,
        "-- Your SQL goes here
CREATE TABLE `forge_reviews`(
	`html_url` TEXT NOT NULL,
	`number` BIGINT NOT NULL PRIMARY KEY,
	`title` TEXT NOT NULL,
	`body` TEXT,
	`author` TEXT,
	`labels` TEXT NOT NULL,
	`draft` BOOL NOT NULL,
	`source_branch` TEXT NOT NULL,
	`target_branch` TEXT NOT NULL,
	`sha` TEXT NOT NULL,
	`created_at` TIMESTAMP,
	`modified_at` TIMESTAMP,
	`merged_at` TIMESTAMP,
	`closed_at` TIMESTAMP,
	`repository_ssh_url` TEXT,
	`repository_https_url` TEXT,
	`repo_owner` TEXT,
	`reviewers` TEXT NOT NULL,
	`unit_symbol` TEXT NOT NULL,
	`last_sync_at` TIMESTAMP NOT NULL,
	`struct_version` INTEGER NOT NULL
);",
    ),
    M::up(
        20260618093000,
        SchemaVersion::Zero,
        "ALTER TABLE `forge_reviews` ADD COLUMN `head_repo_is_fork` BOOL NOT NULL DEFAULT FALSE;",
    ),
    M::up(
        20260624170000,
        SchemaVersion::Zero,
        "ALTER TABLE `forge_reviews` ADD COLUMN `integration_commit_shas` TEXT NOT NULL DEFAULT '[]';
DELETE FROM `forge_reviews`;",
    ),
    M::up(
        20260805120000,
        SchemaVersion::Zero,
        "ALTER TABLE `forge_reviews` ADD COLUMN `auto_merge_enabled` BOOL NOT NULL DEFAULT FALSE;",
    ),
];

/// Tests are in `but-db/tests/db/table/forge_review.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgeReview {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<String>,
    pub labels: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub integration_commit_shas: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub merged_at: Option<chrono::NaiveDateTime>,
    pub closed_at: Option<chrono::NaiveDateTime>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub head_repo_is_fork: bool,
    pub reviewers: String,
    pub auto_merge_enabled: bool,
    pub unit_symbol: String,
    pub last_sync_at: chrono::NaiveDateTime,
    pub struct_version: i32,
}

impl DbHandle {
    pub fn forge_reviews(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: &self.conn }
    }

    pub fn forge_reviews_mut(&mut self) -> rusqlite::Result<ForgeReviewsHandleMut<'_>> {
        Ok(ForgeReviewsHandleMut {
            sp: self.conn.savepoint()?,
        })
    }
}

impl<'conn> Transaction<'conn> {
    pub fn forge_reviews(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: self.inner() }
    }

    pub fn forge_reviews_mut(&mut self) -> rusqlite::Result<ForgeReviewsHandleMut<'_>> {
        Ok(ForgeReviewsHandleMut {
            sp: self.inner_mut().savepoint()?,
        })
    }
}

pub struct ForgeReviewsHandle<'conn> {
    conn: &'conn rusqlite::Connection,
}

pub struct ForgeReviewsHandleMut<'conn> {
    sp: rusqlite::Savepoint<'conn>,
}

impl ForgeReviewsHandle<'_> {
    /// Lists all forge reviews in the database.
    pub fn list_all(&self) -> rusqlite::Result<Vec<ForgeReview>> {
        let mut stmt = self.conn.prepare(
            "SELECT html_url, number, title, body, author, labels, draft, source_branch, \
             target_branch, sha, integration_commit_shas, created_at, modified_at, merged_at, closed_at, \
             repository_ssh_url, repository_https_url, repo_owner, head_repo_is_fork, reviewers, \
             auto_merge_enabled, unit_symbol, last_sync_at, struct_version FROM forge_reviews",
        )?;

        let results = stmt.query_map([], |row| {
            Ok(ForgeReview {
                html_url: row.get(0)?,
                number: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                author: row.get(4)?,
                labels: row.get(5)?,
                draft: row.get(6)?,
                source_branch: row.get(7)?,
                target_branch: row.get(8)?,
                sha: row.get(9)?,
                integration_commit_shas: row.get(10)?,
                created_at: row.get(11)?,
                modified_at: row.get(12)?,
                merged_at: row.get(13)?,
                closed_at: row.get(14)?,
                repository_ssh_url: row.get(15)?,
                repository_https_url: row.get(16)?,
                repo_owner: row.get(17)?,
                head_repo_is_fork: row.get(18)?,
                reviewers: row.get(19)?,
                auto_merge_enabled: row.get(20)?,
                unit_symbol: row.get(21)?,
                last_sync_at: row.get(22)?,
                struct_version: row.get(23)?,
            })
        })?;

        results.collect::<Result<Vec<_>, _>>()
    }
}

impl ForgeReviewsHandleMut<'_> {
    /// Enable read-only access functions.
    pub fn to_ref(&self) -> ForgeReviewsHandle<'_> {
        ForgeReviewsHandle { conn: &self.sp }
    }

    /// Sets the forge_reviews table to the provided values.
    /// Any existing entries that are not in the provided values are deleted.
    pub fn set_all(self, reviews: Vec<ForgeReview>) -> rusqlite::Result<()> {
        self.sp.execute("DELETE FROM forge_reviews", [])?;

        for review in reviews {
            self.upsert_without_commit(review)?;
        }

        self.sp.commit()?;
        Ok(())
    }

    /// Inserts or updates a single forge review by review number.
    pub fn upsert(self, review: ForgeReview) -> rusqlite::Result<()> {
        self.upsert_without_commit(review)?;

        self.sp.commit()?;
        Ok(())
    }

    /// Fold a fresh forge "list reviews" response into the cache.
    ///
    /// The list is the source of truth for which reviews are currently *open*,
    /// with two deliberate exceptions that keep the cache useful — and
    /// non-flickery — between syncs. The four steps run in a single savepoint,
    /// so a partial reconcile is never observable.
    ///
    /// 1. **Delete rows with another `struct_version`.** Their missing fields
    ///    cannot be reconstructed from persisted data.
    /// 2. **Upsert everything listed.** Each listed review is inserted or
    ///    refreshed, which also bumps its `last_sync_at`.
    /// 3. **Delete open reviews the forge no longer lists — but only stale ones**
    ///    (see [`delete_open_reviews_absent_from_list`]). A freshly-written
    ///    optimistic insert is spared via `absent_open_cutoff` so it doesn't
    ///    flicker off before the forge's eventually-consistent list catches up.
    /// 4. **Prune old merged reviews** past `merged_cutoff` (see
    ///    [`prune_merged_before`]). Closed (non-merged) reviews are always kept,
    ///    so direct lookups such as upstream-integration hints keep working.
    ///
    /// [`delete_open_reviews_absent_from_list`]: Self::delete_open_reviews_absent_from_list
    /// [`prune_merged_before`]: Self::prune_merged_before
    pub fn reconcile_listed(
        self,
        reviews: Vec<ForgeReview>,
        struct_version: i32,
        merged_cutoff: chrono::NaiveDateTime,
        absent_open_cutoff: chrono::NaiveDateTime,
    ) -> rusqlite::Result<()> {
        self.sp.execute(
            "DELETE FROM forge_reviews WHERE struct_version != ?1",
            [struct_version],
        )?;
        let listed_numbers = reviews
            .iter()
            .map(|review| review.number)
            .collect::<Vec<_>>();

        for review in reviews {
            self.upsert_without_commit(review)?;
        }
        self.delete_open_reviews_absent_from_list(&listed_numbers, absent_open_cutoff)?;
        self.prune_merged_before(merged_cutoff)?;

        self.sp.commit()?;
        Ok(())
    }

    /// Delete cached *open* reviews (neither merged nor closed) that are absent
    /// from the latest list, restricted to rows last synced at or before
    /// `absent_open_cutoff`.
    ///
    /// The cutoff is what spares a freshly-written optimistic insert (e.g. a PR
    /// just created locally) from being reconciled away before the forge's
    /// eventually-consistent list reflects it: callers pass `now - grace`, so any
    /// row whose `last_sync_at` is newer than the cutoff is retained. An empty
    /// `listed_numbers` means the forge reports no open reviews at all, so every
    /// open review older than the cutoff is eligible for deletion.
    fn delete_open_reviews_absent_from_list(
        &self,
        listed_numbers: &[i64],
        absent_open_cutoff: chrono::NaiveDateTime,
    ) -> rusqlite::Result<()> {
        if listed_numbers.is_empty() {
            self.sp.execute(
                "DELETE FROM forge_reviews \
                 WHERE merged_at IS NULL AND closed_at IS NULL \
                 AND last_sync_at <= ?1",
                [absent_open_cutoff],
            )?;
            return Ok(());
        }

        let placeholders = (1..=listed_numbers.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let cutoff_parameter = listed_numbers.len() + 1;
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(listed_numbers.len() + 1);
        for number in listed_numbers {
            params.push(number);
        }
        params.push(&absent_open_cutoff);

        self.sp.execute(
            &format!(
                "DELETE FROM forge_reviews \
                 WHERE merged_at IS NULL AND closed_at IS NULL \
                 AND number NOT IN ({placeholders}) \
                 AND last_sync_at <= ?{cutoff_parameter}"
            ),
            params.as_slice(),
        )?;
        Ok(())
    }

    /// Delete merged reviews whose `merged_at` is at or before `cutoff`. Does not
    /// commit; the caller owns the surrounding savepoint.
    fn prune_merged_before(&self, cutoff: chrono::NaiveDateTime) -> rusqlite::Result<()> {
        self.sp.execute(
            "DELETE FROM forge_reviews WHERE merged_at IS NOT NULL AND merged_at <= ?1",
            [cutoff],
        )?;
        Ok(())
    }

    fn upsert_without_commit(&self, review: ForgeReview) -> rusqlite::Result<()> {
        self.sp.execute(
            "INSERT INTO forge_reviews (html_url, number, title, body, author, labels, draft, \
             source_branch, target_branch, sha, integration_commit_shas, created_at, modified_at, merged_at, closed_at, \
             repository_ssh_url, repository_https_url, repo_owner, head_repo_is_fork, reviewers, \
             auto_merge_enabled, unit_symbol, last_sync_at, struct_version) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24) \
             ON CONFLICT(number) DO UPDATE SET
                html_url = excluded.html_url,
                title = excluded.title,
                body = excluded.body,
                author = excluded.author,
                labels = excluded.labels,
                draft = excluded.draft,
                source_branch = excluded.source_branch,
                target_branch = excluded.target_branch,
                sha = excluded.sha,
                integration_commit_shas = excluded.integration_commit_shas,
                created_at = excluded.created_at,
                modified_at = excluded.modified_at,
                merged_at = excluded.merged_at,
                closed_at = excluded.closed_at,
                repository_ssh_url = excluded.repository_ssh_url,
                repository_https_url = excluded.repository_https_url,
                repo_owner = excluded.repo_owner,
                head_repo_is_fork = excluded.head_repo_is_fork,
                reviewers = excluded.reviewers,
                auto_merge_enabled = excluded.auto_merge_enabled,
                unit_symbol = excluded.unit_symbol,
                last_sync_at = excluded.last_sync_at,
                struct_version = excluded.struct_version",
            rusqlite::params![
                review.html_url,
                review.number,
                review.title,
                review.body,
                review.author,
                review.labels,
                review.draft,
                review.source_branch,
                review.target_branch,
                review.sha,
                review.integration_commit_shas,
                review.created_at,
                review.modified_at,
                review.merged_at,
                review.closed_at,
                review.repository_ssh_url,
                review.repository_https_url,
                review.repo_owner,
                review.head_repo_is_fork,
                review.reviewers,
                review.auto_merge_enabled,
                review.unit_symbol,
                review.last_sync_at,
                review.struct_version,
            ],
        )?;
        Ok(())
    }

    /// Deletes reviews with a merge timestamp at or before `cutoff`.
    pub fn delete_merged_before(self, cutoff: chrono::NaiveDateTime) -> rusqlite::Result<()> {
        self.prune_merged_before(cutoff)?;
        self.sp.commit()?;
        Ok(())
    }
}
